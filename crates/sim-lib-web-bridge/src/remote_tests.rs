use std::sync::{Arc, Mutex};

use sim_kernel::{Cx, Error, EvalReply, Expr, NumberLiteral, Result, Symbol};
use sim_lib_intent::{Origin, intent};
use sim_lib_server::{
    EvalSite, ServerAddress, ServerFrame, eval_request_from_frame,
    register_loopback_transport_endpoint, server_frame_from_reply,
};
use sim_lib_view::{
    LensRegistry, UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, register_universal_default,
};

use crate::{RemoteTransport, Session, SessionStatus, Transport};

use sim_kernel::testing::eager_cx as cx;
use sim_value::build::keyword as sym;

// conformance: RemoteTransport composes web sessions with sim-lib-server frames.

fn registry() -> LensRegistry {
    let mut registry = LensRegistry::new();
    register_universal_default(&mut registry, false);
    registry
}

fn number(value: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: sym("i64"),
        canonical: value.to_owned(),
    })
}

fn lisp_codec() -> Symbol {
    Symbol::qualified("codec", "lisp")
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

#[test]
fn remote_transport_round_trips_through_real_server_transport() {
    let mut cx = cx();
    let codec_id = cx.registry_mut().fresh_codec_id();
    cx.load_lib(&sim_codec_lisp::LispCodecLib::new(codec_id).unwrap())
        .unwrap();
    let registry = registry();
    let address = ServerAddress::InProcess { thread: 14_053 };
    let site = Arc::new(WebSessionServer::new(address.clone(), sym("doc"), doc()));
    let _endpoint = register_loopback_transport_endpoint(address.clone(), site.clone()).unwrap();

    let mut transport = RemoteTransport::local_server_address("in-process:14053", address)
        .with_offered_codecs(vec![lisp_codec()]);
    transport.connect(&mut cx).unwrap();
    assert_eq!(transport.status(), SessionStatus::Connected);

    let mut session = Session::new(transport);
    let initial = session
        .open(
            &mut cx,
            &registry,
            sym("pane-remote"),
            sym("doc"),
            sym(UNIVERSAL_VIEW_ID),
            sym(UNIVERSAL_EDITOR_ID),
        )
        .unwrap();
    sim_lib_scene::validate_scene(&initial).unwrap();

    session
        .submit_intent_at_rendered_revision(&mut cx, &registry, &sym("pane-remote"), &edit_a_to_9())
        .unwrap();
    let updates = session.pump(&mut cx, &registry).unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(
        sim_value::access::field(&site.value(), "a"),
        Some(&number("9"))
    );

    session.transport_mut().disconnect();
    assert_eq!(session.status(), SessionStatus::Disconnected);
    assert!(session.transport_mut().read(&mut cx, &sym("doc")).is_err());
    session.transport_mut().begin_reconnect();
    session.transport_mut().connect(&mut cx).unwrap();
    assert_eq!(session.status(), SessionStatus::Connected);
    assert_eq!(
        sim_value::access::field(
            &session.transport_mut().read(&mut cx, &sym("doc")).unwrap(),
            "a"
        ),
        Some(&number("9"))
    );

    site.set_value(Expr::Map(vec![
        (Expr::Symbol(sym("a")), number("11")),
        (Expr::Symbol(sym("b")), number("2")),
    ]));
    let stale = session
        .submit_intent_at_rendered_revision(&mut cx, &registry, &sym("pane-remote"), &edit_a_to_9())
        .unwrap_err();
    assert!(
        stale.to_string().contains("stale-revision"),
        "unexpected stale error: {stale}"
    );
    assert_eq!(session.status(), SessionStatus::Connected);

    session.transport_mut().close(&mut cx).unwrap();
    assert_eq!(session.status(), SessionStatus::Closed);
}

struct WebSessionServer {
    address: ServerAddress,
    store: Mutex<WebSessionStore>,
}

struct WebSessionStore {
    resource: Symbol,
    value: Expr,
    events: Vec<Symbol>,
}

impl WebSessionServer {
    fn new(address: ServerAddress, resource: Symbol, value: Expr) -> Self {
        Self {
            address,
            store: Mutex::new(WebSessionStore {
                resource,
                value,
                events: Vec::new(),
            }),
        }
    }

    fn value(&self) -> Expr {
        self.store.lock().unwrap().value.clone()
    }

    fn set_value(&self, value: Expr) {
        self.store.lock().unwrap().value = value;
    }
}

impl EvalSite for WebSessionServer {
    fn site_kind(&self) -> &'static str {
        "web-session-test"
    }

    fn address(&self) -> &ServerAddress {
        &self.address
    }

    fn codecs(&self) -> &[Symbol] {
        static CODECS: std::sync::LazyLock<Vec<Symbol>> =
            std::sync::LazyLock::new(|| vec![lisp_codec()]);
        &CODECS
    }

    fn answer(&self, cx: &mut Cx, frame: ServerFrame) -> Result<ServerFrame> {
        let consistency = frame.envelope.consistency;
        let correlate = frame.msg_id;
        let request = eval_request_from_frame(cx, &frame)?;
        let expr = self.answer_expr(request.expr)?;
        let mut reply = server_frame_from_reply(
            cx,
            &frame.codec,
            EvalReply {
                value: cx.factory().expr(expr)?,
                diagnostics: Vec::new(),
                trace: None,
            },
            consistency,
        )?;
        reply.correlate = correlate;
        Ok(reply)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl WebSessionServer {
    fn answer_expr(&self, request: Expr) -> Result<Expr> {
        let op = field_symbol(&request, "op")?;
        match op.as_qualified_str().as_str() {
            "web-session/read" => self.read_expr(&request),
            "web-session/realize" => self.commit_expr(&request, false),
            "web-session/commit" => self.commit_expr(&request, true),
            "web-session/changes" => self.changes_expr(),
            other => Ok(remote_error("unsupported-operation", other)),
        }
    }

    fn read_expr(&self, request: &Expr) -> Result<Expr> {
        let resource = field_symbol(request, "resource")?;
        let store = self.store.lock().unwrap();
        if resource == store.resource {
            Ok(store.value.clone())
        } else {
            Ok(remote_error(
                "unknown-resource",
                &resource.as_qualified_str(),
            ))
        }
    }

    fn commit_expr(&self, request: &Expr, require_expected: bool) -> Result<Expr> {
        let resource = field_symbol(request, "resource")?;
        let operation = field_expr(request, "operation")?;
        let mut store = self.store.lock().unwrap();
        if resource != store.resource {
            return Ok(remote_error(
                "unknown-resource",
                &resource.as_qualified_str(),
            ));
        }
        if require_expected {
            let expected = field_expr(request, "expected-current")?;
            if !matches!(expected, Expr::Nil) && expected != store.value {
                return Ok(remote_error(
                    "stale-revision",
                    "resource changed before commit",
                ));
            }
        }
        let value = set_value_from_operation(&operation)?;
        store.value = value.clone();
        let resource = store.resource.clone();
        store.events.push(resource);
        Ok(value)
    }

    fn changes_expr(&self) -> Result<Expr> {
        let mut store = self.store.lock().unwrap();
        Ok(Expr::List(
            std::mem::take(&mut store.events)
                .into_iter()
                .map(Expr::Symbol)
                .collect(),
        ))
    }
}

fn field_expr(expr: &Expr, name: &str) -> Result<Expr> {
    let Expr::Map(entries) = expr else {
        return Err(Error::TypeMismatch {
            expected: "map",
            found: "non-map",
        });
    };
    entries
        .iter()
        .find_map(|(key, value)| {
            let is_name = matches!(key, Expr::Symbol(symbol) if symbol.name.as_ref() == name);
            is_name.then(|| value.clone())
        })
        .ok_or_else(|| Error::HostError(format!("missing field {name}")))
}

fn field_symbol(expr: &Expr, name: &str) -> Result<Symbol> {
    match field_expr(expr, name)? {
        Expr::Symbol(symbol) => Ok(symbol),
        _ => Err(Error::TypeMismatch {
            expected: "symbol",
            found: "non-symbol",
        }),
    }
}

fn set_value_from_operation(operation: &Expr) -> Result<Expr> {
    let Expr::Map(entries) = operation else {
        return Err(Error::TypeMismatch {
            expected: "operation map",
            found: "non-map",
        });
    };
    entries
        .iter()
        .find_map(|(key, value)| {
            let is_value = matches!(key, Expr::Symbol(symbol) if symbol.name.as_ref() == "value");
            is_value.then(|| value.clone())
        })
        .ok_or_else(|| Error::HostError("set-value operation missing value".to_owned()))
}

fn remote_error(kind: &str, message: &str) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("error")),
            Expr::Symbol(Symbol::qualified("web-session", kind)),
        ),
        (
            Expr::Symbol(Symbol::new("message")),
            Expr::String(message.to_owned()),
        ),
    ])
}
