//! The fabric transport: a session as a `realize` target on an `EvalFabric`.
//!
//! [`FabricTransport`] implements the web-bridge [`Transport`] trait by
//! delegating every commit to a kernel [`EvalFabric`](sim_kernel::EvalFabric).
//! Reads and change events stay local to an in-memory store (exactly like the
//! [`FixtureTransport`](crate::fixture::FixtureTransport)), but a realized
//! operation is turned into an [`EvalRequest`], answered by the fabric's
//! `realize`, and the reply value becomes the resource's new value. This proves
//! "a surface session is a realize target on the fabric" using the existing
//! Session/pump/diff machinery unchanged: the fabric interprets the checked
//! operation and returns the new resource value.
//!
//! This transport does not provide streams; the stream methods fail closed.

use std::collections::BTreeMap;
use std::sync::Arc;

use sim_kernel::{
    Consistency, Cx, DefaultFactory, EagerPolicy, Error, EvalFabricRef, EvalMode, EvalRequest,
    Expr, Result, Symbol,
};
use sim_lib_stream_core::{PushResult, StreamEnvelope, StreamItem, StreamStats};
use sim_lib_view::Operation;

use crate::transport::{
    ChangeEvent, SessionStatus, StreamInspectorRecord, Transport, TransportKind,
};

/// A transport that commits operations by delegating to an
/// [`EvalFabric`](sim_kernel::EvalFabric).
///
/// Reads return locally stored values and change events accumulate locally;
/// [`Transport::realize`] is forwarded to the fabric, whose reply value becomes
/// the new value of the realized resource.
pub struct FabricTransport {
    fabric: EvalFabricRef,
    cx: Cx,
    store: BTreeMap<Symbol, Expr>,
    events: Vec<ChangeEvent>,
    status: SessionStatus,
}

impl FabricTransport {
    /// A connected, empty transport over `fabric` (with its own kernel context).
    pub fn new(fabric: EvalFabricRef) -> Self {
        Self {
            fabric,
            cx: Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory)),
            store: BTreeMap::new(),
            events: Vec::new(),
            status: SessionStatus::Connected,
        }
    }

    /// Seed a resource value (builder form).
    pub fn with(mut self, resource: Symbol, value: Expr) -> Self {
        self.store.insert(resource, value);
        self
    }

    /// Seed or replace a resource value.
    pub fn set(&mut self, resource: Symbol, value: Expr) {
        self.store.insert(resource, value);
    }

    fn no_streams(&self) -> Error {
        Error::HostError("fabric transport does not provide streams".to_owned())
    }
}

impl Transport for FabricTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::Fabric
    }

    fn status(&self) -> SessionStatus {
        self.status
    }

    fn read(&self, resource: &Symbol) -> Result<Expr> {
        self.store
            .get(resource)
            .cloned()
            .ok_or_else(|| Error::UnknownSymbol {
                symbol: resource.clone(),
            })
    }

    fn realize_operation(&mut self, resource: &Symbol, operation: &Operation) -> Result<Expr> {
        let request = operation_to_request(operation);
        let reply = self.fabric.realize(&mut self.cx, request)?;
        let new_value = reply.value.object().as_expr(&mut self.cx)?;
        validate_reply_shape(&mut self.cx, operation, &new_value)?;
        self.store.insert(resource.clone(), new_value.clone());
        self.events.push(ChangeEvent {
            resource: resource.clone(),
        });
        Ok(new_value)
    }

    fn drain_events(&mut self) -> Vec<ChangeEvent> {
        std::mem::take(&mut self.events)
    }

    fn stream_subscribe(&mut self, _stream_id: &Symbol) -> Result<StreamInspectorRecord> {
        Err(self.no_streams())
    }

    fn stream_read(&mut self, _stream_id: &Symbol, _limit: usize) -> Result<Vec<StreamItem>> {
        Err(self.no_streams())
    }

    fn stream_push(
        &mut self,
        _stream_id: &Symbol,
        _envelope: StreamEnvelope,
    ) -> Result<PushResult> {
        Err(self.no_streams())
    }

    fn stream_cancel(&mut self, _stream_id: &Symbol) -> Result<()> {
        Err(self.no_streams())
    }

    fn stream_stats(&self, _stream_id: &Symbol) -> Result<StreamStats> {
        Err(self.no_streams())
    }

    fn stream_inspector(&self, _stream_id: &Symbol) -> Result<StreamInspectorRecord> {
        Err(self.no_streams())
    }
}

/// Build a default [`EvalRequest`] carrying `operation` and its authority
/// metadata.
///
/// The request uses [`Consistency::LocalFirst`] and [`EvalMode::Eval`], carries
/// the operation's expected result shape and required capabilities, and leaves
/// deadline, streaming, and trace unset. The fabric interprets the operation and
/// returns the resource's new value as the reply value.
pub fn operation_to_request(operation: &Operation) -> EvalRequest {
    EvalRequest {
        expr: operation.form.clone(),
        result_shape: operation.result_shape.clone(),
        required_capabilities: operation.required_capabilities.clone(),
        deadline: None,
        consistency: Consistency::LocalFirst,
        mode: EvalMode::Eval,
        answer_limit: None,
        stream_buffer: None,
        stream: false,
        trace: false,
    }
}

fn validate_reply_shape(cx: &mut Cx, operation: &Operation, value: &Expr) -> Result<()> {
    let Some(shape_value) = &operation.result_shape else {
        return Ok(());
    };
    let Some(shape) = shape_value.object().as_shape() else {
        return Err(Error::HostError(
            "operation result_shape is not a Shape".to_owned(),
        ));
    };
    let matched = shape.check_expr(cx, value)?;
    if matched.accepted {
        Ok(())
    } else {
        Err(Error::HostError(
            "fabric reply failed operation result_shape".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sim_kernel::{
        CapabilityName, Cx, Error, EvalFabric, EvalReply, EvalRequest, Expr, ExprKind,
        NumberLiteral, Result, Symbol,
    };
    use sim_lib_intent::{Origin, intent};
    use sim_lib_view::{
        LensRegistry, Operation, UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, register_universal_default,
    };
    use sim_shape::{ExprKindShape, shape_value};

    use super::{FabricTransport, operation_to_request};
    use crate::session::Session;
    use crate::transport::{Transport, TransportKind};

    /// A fabric that interprets the universal editor's `set-value` operation by
    /// returning its `value` field as the reply value.
    struct SetValueFabric;

    impl EvalFabric for SetValueFabric {
        fn realize(&self, cx: &mut Cx, request: EvalRequest) -> Result<EvalReply> {
            let Expr::Map(entries) = &request.expr else {
                return Err(Error::HostError("operation is not a map".to_owned()));
            };
            let value_expr = sim_value::access::entry_field(entries, "value").ok_or_else(|| {
                Error::HostError("set-value operation is missing a 'value'".to_owned())
            })?;
            Ok(EvalReply {
                value: cx.factory().expr(value_expr.clone())?,
                diagnostics: Vec::new(),
                trace: None,
            })
        }
    }

    struct StringReplyFabric;

    impl EvalFabric for StringReplyFabric {
        fn realize(&self, cx: &mut Cx, _request: EvalRequest) -> Result<EvalReply> {
            Ok(EvalReply {
                value: cx.factory().expr(Expr::String("wrong shape".to_owned()))?,
                diagnostics: Vec::new(),
                trace: None,
            })
        }
    }

    use sim_kernel::testing::eager_cx as cx;

    fn registry() -> LensRegistry {
        let mut registry = LensRegistry::new();
        register_universal_default(&mut registry, false);
        registry
    }

    use sim_value::build::keyword as sym;

    fn number(value: &str) -> Expr {
        Expr::Number(NumberLiteral {
            domain: sym("i64"),
            canonical: value.to_owned(),
        })
    }

    fn doc() -> Expr {
        Expr::Map(vec![
            (Expr::Symbol(sym("a")), number("1")),
            (Expr::Symbol(sym("b")), number("2")),
        ])
    }

    fn edit_a_to_9() -> Expr {
        intent(
            "edit-field",
            Origin::human(1),
            vec![
                ("target", doc()),
                (
                    "path",
                    Expr::List(vec![Expr::Vector(vec![
                        Expr::Symbol(sym("k")),
                        Expr::Symbol(sym("a")),
                    ])]),
                ),
                ("value", number("9")),
            ],
        )
    }

    fn set_value_op(value: Expr) -> Expr {
        Expr::Map(vec![
            (Expr::Symbol(sym("op")), Expr::Symbol(sym("set-value"))),
            (Expr::Symbol(sym("value")), value),
        ])
    }

    fn number_shape() -> sim_kernel::ShapeRef {
        shape_value(
            Symbol::qualified("core", "Number"),
            Arc::new(ExprKindShape::new(ExprKind::Number)),
        )
    }

    #[test]
    fn session_commits_an_edit_through_the_fabric_and_the_scene_diff_reconstructs() {
        let mut cx = cx();
        let registry = registry();
        let transport = FabricTransport::new(Arc::new(SetValueFabric)).with(sym("doc"), doc());
        let mut session = Session::new(transport);

        let initial = session
            .open(
                &mut cx,
                &registry,
                sym("pane-1"),
                sym("doc"),
                sym(UNIVERSAL_VIEW_ID),
                sym(UNIVERSAL_EDITOR_ID),
            )
            .unwrap();
        sim_lib_scene::validate_scene(&initial).expect("initial scene is valid");

        // The Intent decodes and commits through the fabric's realize.
        session
            .submit_intent(&mut cx, &registry, &sym("pane-1"), &edit_a_to_9())
            .unwrap();

        // The fabric-stored value changed.
        let value = session.transport_mut().read(&sym("doc")).unwrap();
        assert_eq!(sim_value::access::field(&value, "a"), Some(&number("9")));

        // Pumping yields a diff that reconstructs the new Scene from the old one.
        let updates = session.pump(&mut cx, &registry).unwrap();
        assert_eq!(updates.len(), 1, "exactly the subscribed pane updates");
        let update = &updates[0];
        assert_eq!(update.pane, sym("pane-1"));
        assert_ne!(update.scene, initial, "the Scene changed");
        let rebuilt = sim_lib_scene::apply(&initial, &update.diff).unwrap();
        assert_eq!(rebuilt, update.scene, "the diff reconstructs the new Scene");
    }

    #[test]
    fn direct_realize_returns_the_new_value_and_records_one_event() {
        let mut transport = FabricTransport::new(Arc::new(SetValueFabric));
        assert_eq!(transport.kind(), TransportKind::Fabric);

        let new_value = transport
            .realize(&sym("x"), &set_value_op(number("42")))
            .unwrap();
        assert_eq!(new_value, number("42"));
        assert_eq!(transport.read(&sym("x")).unwrap(), number("42"));

        let events = transport.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].resource, sym("x"));
        assert!(transport.drain_events().is_empty());
    }

    #[test]
    fn operation_to_request_preserves_shape_and_capability_requirements() {
        let operation = Operation::new(set_value_op(number("42")))
            .with_result_shape(number_shape())
            .requiring(CapabilityName::new("web.write"));

        let request = operation_to_request(&operation);

        assert_eq!(request.expr, operation.form);
        assert!(request.result_shape.is_some());
        assert_eq!(
            request
                .required_capabilities
                .iter()
                .map(|capability| capability.as_str())
                .collect::<Vec<_>>(),
            vec!["web.write"]
        );
    }

    #[test]
    fn fabric_reply_must_match_the_operation_result_shape_before_storage_changes() {
        let mut transport =
            FabricTransport::new(Arc::new(StringReplyFabric)).with(sym("doc"), number("1"));
        let operation = Operation::new(set_value_op(number("2"))).with_result_shape(number_shape());

        let err = transport
            .realize_operation(&sym("doc"), &operation)
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("fabric reply failed operation result_shape"),
            "unexpected error: {err}"
        );
        assert_eq!(transport.read(&sym("doc")).unwrap(), number("1"));
        assert!(transport.drain_events().is_empty());
    }
}
