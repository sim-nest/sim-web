//! Phone and desktop host wrappers over the session bus (VIEW4.09).
//!
//! These are thin facades over [`Session`]: they reuse the same Intent/Scene
//! bus, transport, and pump, and add only the host-shaped policy each device
//! needs. Nothing here re-implements transport, rendering, or diffing.
//!
//! - [`PhoneHost`] is a single-pane facade that caches the last rendered frame
//!   and, while the transport is offline, QUEUES Intents and replays them on
//!   [`PhoneHost::resume`] -- an offline-safe phone that never drops an edit.
//! - [`DesktopHost`] is a many-pane facade: it opens several panes/windows over
//!   one session, so an edit in one pane fans out (through [`Session::pump`]) to
//!   every pane that shares the edited resource.
//!
//! Both stay generic over `T: [`Transport`]`, so they drive the deterministic
//! [`FixtureTransport`](crate::fixture::FixtureTransport) in tests and a real
//! transport later without change.

use std::collections::BTreeMap;

use sim_kernel::{Cx, Error, Expr, Result, Symbol};
use sim_lib_view::surface::SurfaceCaps;
use sim_lib_view::{LensRegistry, UNIVERSAL_SURFACE_CODEC_ID, surface};

use crate::session::{SceneUpdate, Session};
use crate::transport::{SessionStatus, Transport};

/// The single pane a [`PhoneHost`] renders into.
///
/// A phone shows one resource at a time; this is the pane name
/// [`PhoneHost::open`] subscribes and the one to pass to
/// [`PhoneHost::last_scene`].
pub const PHONE_PANE: &str = "phone:main";

/// The maximum number of offline Intents a phone host may buffer.
pub const MAX_PHONE_OFFLINE_QUEUE: usize = 128;

fn universal_surface_codec() -> Symbol {
    Symbol::new(UNIVERSAL_SURFACE_CODEC_ID)
}

/// A phone host facade: a single-pane [`Session`] that caches the last rendered
/// frame and queues Intents while offline, flushing them on resume.
///
/// The phone reuses the session bus wholesale. Its only added policy is offline
/// safety: [`PhoneHost::submit`] commits immediately when connected, but parks
/// Intents in an in-memory queue when the transport is down, and
/// [`PhoneHost::resume`] replays that queue in order once the caller has
/// restored the connection.
pub struct PhoneHost<T: Transport> {
    session: Session<T>,
    caps: SurfaceCaps,
    queue: Vec<Expr>,
    scenes: BTreeMap<Symbol, Expr>,
}

impl<T: Transport> PhoneHost<T> {
    /// Starts a phone host over `transport`, adopting the `phone` surface preset.
    pub fn new(transport: T) -> Self {
        Self {
            session: Session::new(transport),
            caps: surface::preset("phone").expect("phone is a known surface preset"),
            queue: Vec::new(),
            scenes: BTreeMap::new(),
        }
    }

    fn pane() -> Symbol {
        Symbol::new(PHONE_PANE)
    }

    /// Opens `resource` into the phone's single pane with the universal lenses,
    /// caches the initial Scene, and returns it.
    pub fn open(&mut self, cx: &mut Cx, registry: &LensRegistry, resource: Symbol) -> Result<Expr> {
        let pane = Self::pane();
        let scene = self.session.open_codec(
            cx,
            registry,
            pane.clone(),
            resource,
            universal_surface_codec(),
            self.caps.clone(),
        )?;
        self.scenes.insert(pane, scene.clone());
        Ok(scene)
    }

    /// Submits an Intent against the open pane.
    ///
    /// When the transport is [`SessionStatus::Connected`], this commits the
    /// Intent and pumps, caching and returning the resulting frames. Otherwise
    /// the Intent is queued offline and an empty update list is returned -- no
    /// error -- so a flaky link never drops or fails an edit.
    pub fn submit(
        &mut self,
        cx: &mut Cx,
        registry: &LensRegistry,
        intent: Expr,
    ) -> Result<Vec<SceneUpdate>> {
        match self.session.status() {
            SessionStatus::Connected => {
                self.session
                    .submit_intent(cx, registry, &Self::pane(), &intent)?;
                let updates = self.session.pump(cx, registry)?;
                self.cache(&updates);
                Ok(updates)
            }
            _ => {
                if self.queue.len() >= MAX_PHONE_OFFLINE_QUEUE {
                    return Err(Error::HostError(format!(
                        "phone offline queue is full ({MAX_PHONE_OFFLINE_QUEUE}); resume before submitting another intent"
                    )));
                }
                self.queue.push(intent);
                Ok(Vec::new())
            }
        }
    }

    /// Drains the offline queue in order, replaying each Intent through the
    /// session, then pumps once and returns the resulting frames.
    ///
    /// The queue is drained incrementally: the front Intent is removed only once
    /// it commits. If a queued Intent fails (for example one that never
    /// validated against the now-current value), the drain stops with that
    /// Intent still at the front of the queue and the unprocessed tail intact --
    /// no edit is lost, and a later [`resume`](Self::resume) retries from there.
    /// Frames for the edits that did commit are pumped, cached, and returned; if
    /// nothing committed before the failure the error is propagated.
    ///
    /// Reconnecting the underlying transport is the caller's concern; reach it
    /// via [`PhoneHost::transport_mut`] before calling this.
    pub fn resume(&mut self, cx: &mut Cx, registry: &LensRegistry) -> Result<Vec<SceneUpdate>> {
        let pane = Self::pane();
        let mut applied = 0usize;
        let mut failure = None;
        while let Some(intent) = self.queue.first().cloned() {
            match self.session.submit_intent(cx, registry, &pane, &intent) {
                Ok(()) => {
                    self.queue.remove(0);
                    applied += 1;
                }
                Err(err) => {
                    // Leave the failed Intent and the rest of the queue in place,
                    // in order, so the edit is retried rather than dropped.
                    failure = Some(err);
                    break;
                }
            }
        }
        if applied == 0
            && let Some(err) = failure
        {
            return Err(err);
        }
        let updates = self.session.pump(cx, registry)?;
        self.cache(&updates);
        Ok(updates)
    }

    fn cache(&mut self, updates: &[SceneUpdate]) {
        for update in updates {
            self.scenes
                .insert(update.pane.clone(), update.scene.clone());
        }
    }

    /// The phone's advertised surface capabilities (the `phone` preset).
    pub fn caps(&self) -> &SurfaceCaps {
        &self.caps
    }

    /// The number of Intents waiting in the offline queue.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// The most recently cached Scene for `pane`, if one was rendered.
    pub fn last_scene(&self, pane: &Symbol) -> Option<&Expr> {
        self.scenes.get(pane)
    }

    /// Mutable access to the underlying transport, e.g. to drive reconnection.
    pub fn transport_mut(&mut self) -> &mut T {
        self.session.transport_mut()
    }
}

/// A desktop host facade: many panes/windows over one [`Session`].
///
/// The desktop reuses one session for every open pane, so panes that share a
/// resource stay coherent for free: an edit submitted on one pane fans out
/// through [`Session::pump`] to every pane subscribed to the same resource.
pub struct DesktopHost<T: Transport> {
    session: Session<T>,
    caps: SurfaceCaps,
    panes: Vec<Symbol>,
}

impl<T: Transport> DesktopHost<T> {
    /// Starts a desktop host over `transport`, adopting the `desktop` preset.
    pub fn new(transport: T) -> Self {
        Self {
            session: Session::new(transport),
            caps: surface::preset("desktop").expect("desktop is a known surface preset"),
            panes: Vec::new(),
        }
    }

    /// Opens `resource` into the named `pane` with the universal lenses, tracks
    /// the pane, and returns its initial Scene.
    ///
    /// Opening the same resource into several panes subscribes each of them;
    /// re-opening an already-tracked pane re-subscribes it without duplicating
    /// it in [`DesktopHost::panes`].
    pub fn open_pane(
        &mut self,
        cx: &mut Cx,
        registry: &LensRegistry,
        pane: Symbol,
        resource: Symbol,
    ) -> Result<Expr> {
        let scene = self.session.open_codec(
            cx,
            registry,
            pane.clone(),
            resource,
            universal_surface_codec(),
            self.caps.clone(),
        )?;
        if !self.panes.contains(&pane) {
            self.panes.push(pane);
        }
        Ok(scene)
    }

    /// Submits an Intent against `pane` and pumps.
    ///
    /// The returned updates may span several panes when they share the edited
    /// resource.
    pub fn submit(
        &mut self,
        cx: &mut Cx,
        registry: &LensRegistry,
        pane: &Symbol,
        intent: Expr,
    ) -> Result<Vec<SceneUpdate>> {
        self.session.submit_intent(cx, registry, pane, &intent)?;
        self.session.pump(cx, registry)
    }

    /// The panes currently open, in the order they were first opened.
    pub fn panes(&self) -> Vec<Symbol> {
        self.panes.clone()
    }

    /// The desktop's advertised surface capabilities (the `desktop` preset).
    pub fn caps(&self) -> &SurfaceCaps {
        &self.caps
    }

    /// Mutable access to the underlying transport, e.g. to drive reconnection.
    pub fn transport_mut(&mut self) -> &mut T {
        self.session.transport_mut()
    }
}

#[cfg(test)]
mod tests {

    use sim_kernel::{Expr, NumberLiteral, Symbol};
    use sim_lib_intent::{Origin, intent};
    use sim_lib_view::{LensRegistry, register_universal_default};

    use super::{DesktopHost, MAX_PHONE_OFFLINE_QUEUE, PHONE_PANE, PhoneHost};
    use crate::fixture::FixtureTransport;
    use crate::transport::Transport;

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

    /// An `edit-field` Intent that sets map field `name` to `value`.
    fn edit(name: &str, value: &str) -> Expr {
        intent(
            "edit-field",
            Origin::human(1),
            vec![
                ("target", doc()),
                (
                    "path",
                    Expr::List(vec![Expr::Vector(vec![
                        Expr::Symbol(sym("k")),
                        Expr::Symbol(sym(name)),
                    ])]),
                ),
                ("value", number(value)),
            ],
        )
    }

    /// A structurally valid `edit-field` Intent whose path indexes into the map
    /// as if it were a sequence, so `set_at` rejects it and the editor refuses to
    /// commit -- a queued Intent that fails on replay.
    fn broken_edit() -> Expr {
        intent(
            "edit-field",
            Origin::human(1),
            vec![
                ("target", doc()),
                (
                    "path",
                    Expr::List(vec![Expr::Vector(vec![
                        Expr::Symbol(sym("i")),
                        Expr::String("0".to_owned()),
                    ])]),
                ),
                ("value", number("99")),
            ],
        )
    }

    fn field_of(value: &Expr, name: &str) -> Option<Expr> {
        let Expr::Map(entries) = value else {
            return None;
        };
        entries
            .iter()
            .find(|(k, _)| matches!(k, Expr::Symbol(s) if &*s.name == name))
            .map(|(_, v)| v.clone())
    }

    #[test]
    fn phone_caches_online_edits_and_queues_offline_ones_until_resume() {
        let mut cx = cx();
        let registry = registry();
        let mut phone = PhoneHost::new(FixtureTransport::new().with(sym("doc"), doc()));
        let pane = sym(PHONE_PANE);

        // Open and render the resource.
        let initial = phone.open(&mut cx, &registry, sym("doc")).unwrap();
        sim_lib_scene::validate_scene(&initial).expect("initial scene is valid");
        assert_eq!(phone.last_scene(&pane), Some(&initial));

        // A connected edit commits, pumps, and caches the new frame.
        let online = phone.submit(&mut cx, &registry, edit("a", "9")).unwrap();
        assert_eq!(online.len(), 1, "the open pane updates");
        assert_eq!(phone.queued(), 0, "nothing is queued while connected");
        assert_eq!(phone.last_scene(&pane), Some(&online[0].scene));
        assert_ne!(online[0].scene, initial, "the frame changed");

        // Offline: two edits queue instead of committing -- no error, no frames.
        phone.transport_mut().disconnect();
        let q1 = phone.submit(&mut cx, &registry, edit("b", "8")).unwrap();
        let q2 = phone.submit(&mut cx, &registry, edit("a", "30")).unwrap();
        assert!(
            q1.is_empty() && q2.is_empty(),
            "offline edits return no frames"
        );
        assert_eq!(phone.queued(), 2, "both offline edits are queued");

        // Reconnect and resume: the queued edits replay in order.
        phone.transport_mut().begin_reconnect();
        phone.transport_mut().reconnect();
        let resumed = phone.resume(&mut cx, &registry).unwrap();
        assert_eq!(phone.queued(), 0, "the queue drained");
        assert_eq!(resumed.len(), 2, "one frame per replayed edit, in order");

        // The final value reflects BOTH queued edits (b := 8 then a := 30).
        let value = phone.transport_mut().read(&sym("doc")).unwrap();
        assert_eq!(field_of(&value, "a"), Some(number("30")));
        assert_eq!(field_of(&value, "b"), Some(number("8")));

        // last_scene reflects the latest frame.
        let latest = resumed.last().expect("resume produced frames");
        assert_eq!(phone.last_scene(&pane), Some(&latest.scene));
    }

    #[test]
    fn resume_stops_at_a_failing_intent_and_keeps_the_tail() {
        let mut cx = cx();
        let registry = registry();
        let mut phone = PhoneHost::new(FixtureTransport::new().with(sym("doc"), doc()));
        phone.open(&mut cx, &registry, sym("doc")).unwrap();

        // Offline: queue a good edit, a broken Intent, then another good edit.
        phone.transport_mut().disconnect();
        phone.submit(&mut cx, &registry, edit("b", "8")).unwrap();
        phone.submit(&mut cx, &registry, broken_edit()).unwrap();
        phone.submit(&mut cx, &registry, edit("a", "30")).unwrap();
        assert_eq!(phone.queued(), 3, "all three edits are queued offline");

        // Reconnect and resume: the first edit commits, the broken Intent halts
        // the drain, and the broken+trailing edits stay queued in order.
        phone.transport_mut().begin_reconnect();
        phone.transport_mut().reconnect();
        let updates = phone.resume(&mut cx, &registry).unwrap();
        assert!(!updates.is_empty(), "the committed edit produced a frame");
        assert_eq!(
            phone.queued(),
            2,
            "the failed Intent and its tail are NOT dropped"
        );

        // Only the first edit took effect; the trailing edit never ran.
        let value = phone.transport_mut().read(&sym("doc")).unwrap();
        assert_eq!(field_of(&value, "b"), Some(number("8")), "b := 8 applied");
        assert_eq!(
            field_of(&value, "a"),
            Some(number("1")),
            "a is untouched -- the post-failure edit did not apply"
        );
    }

    #[test]
    fn phone_offline_queue_has_a_backpressure_cap() {
        let mut cx = cx();
        let registry = registry();
        let mut phone = PhoneHost::new(FixtureTransport::new().with(sym("doc"), doc()));
        phone.open(&mut cx, &registry, sym("doc")).unwrap();
        phone.transport_mut().disconnect();

        for index in 0..MAX_PHONE_OFFLINE_QUEUE {
            phone
                .submit(&mut cx, &registry, edit("a", &index.to_string()))
                .unwrap();
        }

        let err = phone
            .submit(&mut cx, &registry, edit("a", "999"))
            .unwrap_err();

        assert!(
            err.to_string().contains("phone offline queue is full"),
            "unexpected error: {err}"
        );
        assert_eq!(phone.queued(), MAX_PHONE_OFFLINE_QUEUE);
    }

    #[test]
    fn desktop_fans_a_shared_resource_edit_out_to_every_pane() {
        let mut cx = cx();
        let registry = registry();
        let mut desktop = DesktopHost::new(FixtureTransport::new().with(sym("doc"), doc()));

        // Open the SAME resource into two panes.
        let scene_a = desktop
            .open_pane(&mut cx, &registry, sym("pane-a"), sym("doc"))
            .unwrap();
        let scene_b = desktop
            .open_pane(&mut cx, &registry, sym("pane-b"), sym("doc"))
            .unwrap();
        assert_eq!(desktop.panes(), vec![sym("pane-a"), sym("pane-b")]);

        // Edit on pane A; pump fans out to BOTH panes sharing the resource.
        let updates = desktop
            .submit(&mut cx, &registry, &sym("pane-a"), edit("a", "9"))
            .unwrap();
        assert_eq!(updates.len(), 2, "both panes share the resource");
        let panes: Vec<Symbol> = updates.iter().map(|u| u.pane.clone()).collect();
        assert!(panes.contains(&sym("pane-a")) && panes.contains(&sym("pane-b")));

        // Each pane's diff reconstructs its new Scene from its initial one.
        for update in &updates {
            let initial = if update.pane == sym("pane-a") {
                &scene_a
            } else {
                &scene_b
            };
            let rebuilt = sim_lib_scene::apply(initial, &update.diff).unwrap();
            assert_eq!(rebuilt, update.scene, "the diff reconstructs the new Scene");
        }
    }
}
