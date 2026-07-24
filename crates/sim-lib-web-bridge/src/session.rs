//! The session: the Intent/Scene bus with per-pane subscriptions.
//!
//! A session ties panes to resources over a [`Transport`]. Opening a value
//! renders its Scene and subscribes the pane; submitting an Intent decodes it
//! through the pane's surface codec, commits the operation through `realize`,
//! and the transport records a change; pumping re-renders only the affected
//! panes and returns a Scene diff (from P1) for each. The session never speaks a
//! transport-specific API.

use sim_kernel::{Cx, Error, Expr, Result, Symbol};
use sim_lib_view::{
    LensRegistry, Mode, SurfaceCaps, UNIVERSAL_SURFACE_CODEC_ID, surface, universal_scene,
};

use crate::transport::{SessionStatus, Transport};

/// The largest number of distinct panes one session may hold at once. Opening
/// beyond this is refused: untrusted `pane` query values must not grow the
/// per-pane work [`Session::pump`] does on every event without bound.
const MAX_PANES: usize = 64;

/// The largest accepted pane-name length, bounding an untrusted `pane` value.
const MAX_PANE_NAME: usize = 128;

/// The largest accepted resource-name length, bounding an untrusted `resource`
/// value.
const MAX_RESOURCE_NAME: usize = 512;

/// Reject a pane name that is empty, over-long, or not printable ASCII (the
/// `pane` query param is untrusted).
fn validate_pane_name(pane: &Symbol) -> Result<()> {
    let name = pane.as_qualified_str();
    if name.is_empty() || name.len() > MAX_PANE_NAME {
        return Err(Error::HostError(format!(
            "pane name must be 1..={MAX_PANE_NAME} bytes, got {}",
            name.len()
        )));
    }
    if !name.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(Error::HostError(
            "pane name must be printable ASCII without spaces".to_owned(),
        ));
    }
    Ok(())
}

/// Reject a resource name that is empty or over-long (the `resource` query
/// param is untrusted). Charset stays lenient; an unknown resource fails the
/// transport read anyway.
fn validate_resource_name(resource: &Symbol) -> Result<()> {
    let name = resource.as_qualified_str();
    if name.is_empty() || name.len() > MAX_RESOURCE_NAME {
        return Err(Error::HostError(format!(
            "resource name must be 1..={MAX_RESOURCE_NAME} bytes, got {}",
            name.len()
        )));
    }
    Ok(())
}

/// A live binding of a pane to a resource and its lenses.
struct Subscription {
    pane: Symbol,
    resource: Symbol,
    codec: Symbol,
    caps: SurfaceCaps,
    rendered_value: Expr,
    last_scene: Expr,
}

/// A re-rendered Scene for a pane, with the diff from its previous Scene.
#[derive(Clone, Debug)]
pub struct SceneUpdate {
    /// The pane that updated.
    pub pane: Symbol,
    /// The full new Scene.
    pub scene: Expr,
    /// The diff from the previous Scene (a `scene/patch` value).
    pub diff: Expr,
}

/// A session over a transport, with per-pane subscriptions and an experience
/// mode. The mode is session state (a value); switching it never changes the
/// values being shown.
pub struct Session<T: Transport> {
    transport: T,
    subscriptions: Vec<Subscription>,
    mode: Mode,
}

impl<T: Transport> Session<T> {
    /// Start a session over `transport` in Builder mode.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            subscriptions: Vec::new(),
            mode: Mode::Builder,
        }
    }

    /// The visible connection status.
    pub fn status(&self) -> SessionStatus {
        self.transport.status()
    }

    /// The active experience mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Handle an `intent/set-mode`, switching the session mode. The values being
    /// shown are never read or written.
    pub fn set_mode(&mut self, intent: &Expr) -> Result<()> {
        match sim_value::access::field(intent, "kind") {
            Some(Expr::Symbol(kind)) if &*kind.name == "set-mode" => {}
            _ => {
                return Err(Error::HostError(
                    "set_mode expects an intent/set-mode".to_owned(),
                ));
            }
        }
        let mode = match sim_value::access::field(intent, "mode") {
            Some(Expr::Symbol(symbol)) => Mode::from_name(&symbol.name),
            _ => None,
        };
        self.mode = mode.ok_or_else(|| {
            Error::HostError(
                "intent/set-mode 'mode' must be household, builder, or systems".to_owned(),
            )
        })?;
        Ok(())
    }

    /// Render a value through the universal default lens at the session's mode
    /// depth (Household/Builder/Systems show progressively more).
    pub fn render_universal(&self, value: &Expr) -> Expr {
        universal_scene(value, self.mode)
    }

    /// Mutable access to the transport (for example to simulate disconnect in
    /// tests, or to drive reconnection).
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Open `resource` into `pane` with a canonical reversible surface codec;
    /// render and subscribe. Returns the initial Scene.
    pub fn open_codec(
        &mut self,
        cx: &mut Cx,
        registry: &LensRegistry,
        pane: Symbol,
        resource: Symbol,
        codec: Symbol,
        caps: SurfaceCaps,
    ) -> Result<Expr> {
        validate_pane_name(&pane)?;
        validate_resource_name(&resource)?;
        let replacing = self.subscriptions.iter().any(|sub| sub.pane == pane);
        if !replacing && self.subscriptions.len() >= MAX_PANES {
            return Err(Error::HostError(format!(
                "session is at its pane limit ({MAX_PANES}); close a pane before opening another"
            )));
        }
        let surface_codec = registry
            .surface_codec(&codec)
            .ok_or_else(|| Error::UnknownSymbol {
                symbol: codec.clone(),
            })?;
        let value = self.transport.read(cx, &resource)?;
        let scene = surface_codec.encode(cx, &value, &caps)?;
        self.subscriptions.retain(|sub| sub.pane != pane);
        self.subscriptions.push(Subscription {
            pane,
            resource,
            codec,
            caps,
            rendered_value: value,
            last_scene: scene.clone(),
        });
        Ok(scene)
    }

    /// Compatibility adapter for callers that still pass split view/editor lens
    /// ids. The bridge session itself stores the canonical
    /// [`SurfaceCodec`](sim_lib_view::SurfaceCodec) id and surface caps.
    pub fn open(
        &mut self,
        cx: &mut Cx,
        registry: &LensRegistry,
        pane: Symbol,
        resource: Symbol,
        _view_lens: Symbol,
        _editor_lens: Symbol,
    ) -> Result<Expr> {
        self.open_codec(
            cx,
            registry,
            pane,
            resource,
            Symbol::new(UNIVERSAL_SURFACE_CODEC_ID),
            surface::preset("desktop").expect("desktop is a known surface preset"),
        )
    }

    /// Submit an Intent against the value shown in `pane`: decode through the
    /// pane's surface codec and commit the operation through `realize`.
    pub fn submit_intent(
        &mut self,
        cx: &mut Cx,
        registry: &LensRegistry,
        pane: &Symbol,
        intent: &Expr,
    ) -> Result<()> {
        self.submit_intent_with_policy(cx, registry, pane, intent, false)
    }

    /// Submit an Intent only if the pane still reflects the value rendered when
    /// it was last opened or pumped. This is the optimistic revision path for
    /// browser clients that include a rendered frame revision in their request.
    pub fn submit_intent_at_rendered_revision(
        &mut self,
        cx: &mut Cx,
        registry: &LensRegistry,
        pane: &Symbol,
        intent: &Expr,
    ) -> Result<()> {
        self.submit_intent_with_policy(cx, registry, pane, intent, true)
    }

    fn submit_intent_with_policy(
        &mut self,
        cx: &mut Cx,
        registry: &LensRegistry,
        pane: &Symbol,
        intent: &Expr,
        require_rendered_revision: bool,
    ) -> Result<()> {
        let (resource, codec, rendered_value) = {
            let sub = self
                .subscriptions
                .iter()
                .find(|sub| &sub.pane == pane)
                .ok_or_else(|| Error::HostError(format!("pane '{pane}' is not open")))?;
            (
                sub.resource.clone(),
                sub.codec.clone(),
                sub.rendered_value.clone(),
            )
        };
        let value = if require_rendered_revision {
            rendered_value.clone()
        } else {
            self.transport.read(cx, &resource)?
        };
        let surface_codec = registry
            .surface_codec(&codec)
            .ok_or(Error::UnknownSymbol { symbol: codec })?;
        let draft = surface_codec.decode(cx, &value, intent)?;
        let operation = surface_codec.commit(cx, &draft)?;
        self.transport.commit_operation(
            cx,
            &resource,
            &operation,
            require_rendered_revision.then_some(&rendered_value),
        )?;
        Ok(())
    }

    /// Drain pending changes and re-render only the affected panes, returning a
    /// Scene update (with diff) for each.
    pub fn pump(&mut self, cx: &mut Cx, registry: &LensRegistry) -> Result<Vec<SceneUpdate>> {
        let events = self.transport.drain_events(cx)?;
        let mut updates = Vec::new();
        let Self {
            transport,
            subscriptions,
            ..
        } = self;
        for event in events {
            for sub in subscriptions
                .iter_mut()
                .filter(|sub| sub.resource == event.resource)
            {
                let value = transport.read(cx, &sub.resource)?;
                let surface_codec =
                    registry
                        .surface_codec(&sub.codec)
                        .ok_or_else(|| Error::UnknownSymbol {
                            symbol: sub.codec.clone(),
                        })?;
                let scene = surface_codec.encode(cx, &value, &sub.caps)?;
                let diff = sim_lib_scene::diff(&sub.last_scene, &scene);
                sub.rendered_value = value;
                sub.last_scene = scene.clone();
                updates.push(SceneUpdate {
                    pane: sub.pane.clone(),
                    scene,
                    diff,
                });
            }
        }
        Ok(updates)
    }
}

#[cfg(test)]
mod tests {

    use sim_kernel::{Cx, Expr, Symbol};
    use sim_lib_view::{
        LensRegistry, UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, register_universal_default,
    };

    use super::{MAX_PANES, Session};
    use crate::fixture::FixtureTransport;

    use sim_value::build::keyword as sym;

    use sim_kernel::testing::eager_cx as cx;

    fn registry() -> LensRegistry {
        let mut registry = LensRegistry::new();
        register_universal_default(&mut registry, false);
        registry
    }

    fn open(
        session: &mut Session<FixtureTransport>,
        cx: &mut Cx,
        registry: &LensRegistry,
        pane: &str,
    ) -> sim_kernel::Result<Expr> {
        session.open(
            cx,
            registry,
            sym(pane),
            sym("doc"),
            Symbol::new(UNIVERSAL_VIEW_ID),
            Symbol::new(UNIVERSAL_EDITOR_ID),
        )
    }

    #[test]
    fn open_bounds_the_number_of_panes() {
        let mut cx = cx();
        let registry = registry();
        let mut session = Session::new(FixtureTransport::new().with(sym("doc"), Expr::Nil));

        for index in 0..MAX_PANES {
            open(&mut session, &mut cx, &registry, &format!("pane-{index}")).unwrap();
        }
        // A new distinct pane beyond the cap is refused.
        assert!(
            open(&mut session, &mut cx, &registry, "pane-overflow").is_err(),
            "opening past the pane cap must be refused"
        );
        // Re-opening an EXISTING pane still works (it replaces, never grows).
        open(&mut session, &mut cx, &registry, "pane-0").unwrap();
    }

    #[test]
    fn open_rejects_untrusted_pane_names() {
        let mut cx = cx();
        let registry = registry();
        let mut session = Session::new(FixtureTransport::new().with(sym("doc"), Expr::Nil));

        assert!(
            open(&mut session, &mut cx, &registry, "").is_err(),
            "empty pane"
        );
        let huge = "p".repeat(super::MAX_PANE_NAME + 1);
        assert!(
            open(&mut session, &mut cx, &registry, &huge).is_err(),
            "over-long pane name"
        );
        assert!(
            open(&mut session, &mut cx, &registry, "has space").is_err(),
            "pane name with a space"
        );
    }
}
