//! Dual-glasses co-use over one synchronized surface hub.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_view::SurfaceCaps;
use sim_lib_view_device::{ConsentReceipt, EdgeId, FrameClock, StalePolicy};
use sim_lib_view_spatial::{
    GlassesCoUseRole, GlassesPeer, HaloGlanceLoop, VitureReprojectLoop, glasses_peer_config,
    halo_loop, viture_loop,
};
use sim_value::{access, build};

use crate::{Broadcast, EditRow, SurfaceBinding, SurfaceHub, SurfaceRole};

const WORKSPACE_PANE: &str = "glasses-workspace";
const DEFAULT_MAX_PREDICT_MS: u64 = 12;

/// One worn-together Viture+Halo session.
///
/// The session owns one [`SurfaceHub`] and one consent receipt. Viture and Halo
/// are independent surface peers in that hub, so a decoded Intent commits once
/// to the canonical workspace and fans out to every attached projection.
pub struct GlassesCoUseSession {
    hub: SurfaceHub,
    session: EdgeId,
    consent: ConsentReceipt,
    resource: Symbol,
    pane: Symbol,
    viture_surface: Symbol,
    halo_surface: Symbol,
    viture_loop: Option<VitureReprojectLoop>,
    halo_loop: Option<HaloGlanceLoop>,
    viture_clock: Option<FrameClock>,
    halo_clock: Option<FrameClock>,
}

impl GlassesCoUseSession {
    /// Builds a co-use session around an initial canonical workspace value.
    pub fn new(
        session: EdgeId,
        consent: ConsentReceipt,
        resource: Symbol,
        initial_workspace: Expr,
    ) -> Result<Self> {
        if consent.session != session {
            return Err(Error::HostError(
                "glasses co-use consent receipt is bound to a different session".to_owned(),
            ));
        }
        let mut hub = SurfaceHub::new();
        hub.seed(resource.clone(), initial_workspace);
        let viture_surface = glasses_surface(&session, GlassesPeer::Viture);
        let halo_surface = glasses_surface(&session, GlassesPeer::Halo);
        Ok(Self {
            hub,
            session,
            consent,
            resource,
            pane: Symbol::new(WORKSPACE_PANE),
            viture_surface,
            halo_surface,
            viture_loop: None,
            halo_loop: None,
            viture_clock: None,
            halo_clock: None,
        })
    }

    /// Attach the Viture main surface and open the shared workspace there.
    pub fn attach_viture(&mut self, caps: SurfaceCaps) -> Result<Expr> {
        let config = glasses_peer_config(GlassesPeer::Viture, &caps)?;
        let (loop_, clock) = viture_loop(&config.profile, DEFAULT_MAX_PREDICT_MS);
        self.hub.register_surface_with_role(
            self.viture_surface.clone(),
            caps,
            surface_role_for(config.role),
        );
        self.viture_loop = Some(loop_);
        self.viture_clock = Some(clock);
        self.hub.open(
            &self.viture_surface,
            self.pane.clone(),
            self.resource.clone(),
        )
    }

    /// Attach the Halo peer surface and open the shared workspace there.
    pub fn attach_halo(&mut self, caps: SurfaceCaps) -> Result<Expr> {
        let config = glasses_peer_config(GlassesPeer::Halo, &caps)?;
        let (loop_, clock) = halo_loop(&config.profile);
        self.hub.register_surface_with_role(
            self.halo_surface.clone(),
            caps,
            surface_role_for(config.role),
        );
        self.halo_loop = Some(loop_);
        self.halo_clock = Some(clock);
        self.hub
            .open(&self.halo_surface, self.pane.clone(), self.resource.clone())
    }

    /// Detach the Viture main surface without ending the shared session.
    pub fn detach_viture(&mut self) -> Vec<SurfaceBinding> {
        self.viture_loop = None;
        self.viture_clock = None;
        self.hub.detach_surface(&self.viture_surface)
    }

    /// Submit a standard editor Intent from the selected glasses peer.
    pub fn submit_from(&mut self, peer: GlassesPeer, intent: &Expr) -> Result<Vec<Broadcast>> {
        let surface = self.surface(peer).clone();
        self.hub.submit(&surface, &self.pane, intent)
    }

    /// Commit a coordinator-decoded value update from the selected glasses peer.
    pub fn commit_from(
        &mut self,
        peer: GlassesPeer,
        intent: &Expr,
        new_workspace: Expr,
    ) -> Result<Vec<Broadcast>> {
        let surface = self.surface(peer).clone();
        self.hub
            .commit_value_from(&surface, &self.pane, intent, new_workspace)
    }

    /// Acknowledge the focused Viture review card from a Halo tap Intent.
    pub fn acknowledge_review_from_halo(
        &mut self,
        tap_intent: &Expr,
        mission: Symbol,
    ) -> Result<Vec<Broadcast>> {
        let current = self.workspace()?.clone();
        let review = access::field(&current, "review")
            .cloned()
            .unwrap_or_else(|| build::map(Vec::new()));
        let review = access::set(&review, "mission", Expr::Symbol(mission));
        let review = access::set(&review, "status", build::sym("acked"));
        let review = access::set(&review, "acked-by", Expr::Symbol(self.halo_surface.clone()));
        let next = access::set(&current, "review", review);
        self.commit_from(GlassesPeer::Halo, tap_intent, next)
    }

    /// Returns the shared device-edge session id.
    pub fn session(&self) -> &EdgeId {
        &self.session
    }

    /// Returns the session-bound consent receipt.
    pub fn consent(&self) -> &ConsentReceipt {
        &self.consent
    }

    /// Returns the canonical shared workspace.
    pub fn workspace(&self) -> Result<&Expr> {
        self.hub.canonical(&self.resource).ok_or_else(|| {
            Error::HostError(format!(
                "glasses workspace '{}' is not seeded",
                self.resource
            ))
        })
    }

    /// Returns the append-only shared edit ledger.
    pub fn ledger(&self) -> &[EditRow] {
        self.hub.ledger()
    }

    /// Returns live bindings currently viewing the shared workspace.
    pub fn live_bindings(&self) -> Vec<SurfaceBinding> {
        self.hub.bindings_for_resource(&self.resource)
    }

    /// Returns the surface id for `peer`.
    pub fn surface(&self, peer: GlassesPeer) -> &Symbol {
        match peer {
            GlassesPeer::Viture => &self.viture_surface,
            GlassesPeer::Halo => &self.halo_surface,
        }
    }

    /// Returns the role recorded for a peer surface.
    pub fn role(&self, peer: GlassesPeer) -> Option<SurfaceRole> {
        self.hub.surface_role(self.surface(peer))
    }

    /// Returns the Viture adapter-loop staleness policy when attached.
    pub fn viture_loop_policy(&self) -> Option<StalePolicy> {
        self.viture_loop.as_ref().map(|loop_| loop_.policy())
    }

    /// Returns the Halo adapter-loop staleness policy when attached.
    pub fn halo_loop_policy(&self) -> Option<StalePolicy> {
        self.halo_loop.as_ref().map(|loop_| loop_.policy())
    }
}

/// Returns the stable surface id for a glasses peer in `session`.
pub fn glasses_surface(session: &EdgeId, peer: GlassesPeer) -> Symbol {
    Symbol::qualified(
        "device/peer",
        format!(
            "{}:{}",
            session.as_symbol().as_qualified_str(),
            peer.label()
        ),
    )
}

fn surface_role_for(role: GlassesCoUseRole) -> SurfaceRole {
    match role {
        GlassesCoUseRole::Main => SurfaceRole::Main,
        GlassesCoUseRole::Peer => SurfaceRole::Peer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sim_lib_intent::{Origin, intent};
    use sim_lib_view_device::{DeviceCapability, StalePolicy};

    fn session() -> GlassesCoUseSession {
        let edge = EdgeId::named("wear-session");
        let consent = ConsentReceipt::new(
            vec![
                DeviceCapability::Pose.grant_symbol(),
                DeviceCapability::Mic.grant_symbol(),
            ],
            60_000,
            Vec::new(),
            edge.clone(),
            7,
        );
        GlassesCoUseSession::new(edge, consent, build::keyword("workspace"), workspace()).unwrap()
    }

    fn workspace() -> Expr {
        build::map(vec![
            ("title", Expr::String("Bridge review".to_owned())),
            ("status", build::sym("pending")),
            ("focus", build::sym("main-panel")),
            (
                "review",
                build::map(vec![
                    ("mission", build::qsym("bridge", "packet-review")),
                    ("status", build::sym("pending")),
                ]),
            ),
        ])
    }

    fn attach_both(session: &mut GlassesCoUseSession) {
        let viture = SurfaceCaps::from_preset("glasses-luma-ultra", "viture.co-use").unwrap();
        let halo = SurfaceCaps::from_preset("glasses-hud", "halo.co-use").unwrap();
        sim_lib_scene::validate_scene(&session.attach_viture(viture).unwrap()).unwrap();
        sim_lib_scene::validate_scene(&session.attach_halo(halo).unwrap()).unwrap();
    }

    fn edit_field(tick: u64, field: &str, value: Expr) -> Expr {
        intent(
            "edit-field",
            Origin::human(tick),
            vec![
                ("target", workspace()),
                (
                    "path",
                    Expr::List(vec![Expr::Vector(vec![
                        Expr::Symbol(build::keyword("k")),
                        Expr::Symbol(build::keyword(field)),
                    ])]),
                ),
                ("value", value),
            ],
        )
    }

    fn invoke(tick: u64, op: &str) -> Expr {
        intent(
            "invoke",
            Origin::human(tick),
            vec![
                ("target", build::sym("workspace")),
                ("op", Expr::Symbol(Symbol::qualified("glasses/input", op))),
                ("args", Expr::List(Vec::new())),
            ],
        )
    }

    #[test]
    fn viture_and_halo_share_one_session() {
        let mut session = session();
        attach_both(&mut session);

        assert_eq!(session.role(GlassesPeer::Viture), Some(SurfaceRole::Main));
        assert_eq!(session.role(GlassesPeer::Halo), Some(SurfaceRole::Peer));
        assert_eq!(
            session.viture_loop_policy(),
            Some(StalePolicy::Predict),
            "Viture keeps its own reprojector loop"
        );
        assert_eq!(
            session.halo_loop_policy(),
            Some(StalePolicy::HoldLast),
            "Halo keeps its own glance loop"
        );
        assert_eq!(session.live_bindings().len(), 2);

        let halo_voice = edit_field(1, "title", Expr::String("Halo voice edit".to_owned()));
        let broadcasts = session
            .submit_from(GlassesPeer::Halo, &halo_voice)
            .expect("Halo ASR edit commits through the hub");
        assert_broadcasts_to_both(&session, &broadcasts);
        assert_eq!(
            access::field(session.workspace().unwrap(), "title"),
            Some(&Expr::String("Halo voice edit".to_owned()))
        );
        assert_eq!(session.ledger().len(), 1);

        let viture_intent = invoke(2, "pinch");
        let next = access::set(
            session.workspace().unwrap(),
            "focus",
            build::sym("viture-hand"),
        );
        let broadcasts = session
            .commit_from(GlassesPeer::Viture, &viture_intent, next)
            .expect("decoded Viture hand Intent commits once");
        assert_broadcasts_to_both(&session, &broadcasts);
        assert_eq!(
            access::field(session.workspace().unwrap(), "focus"),
            Some(&build::sym("viture-hand"))
        );
        assert_eq!(session.ledger().len(), 2);
        assert_eq!(session.ledger()[1].tick, 2);
    }

    #[test]
    fn halo_tap_acks_viture_card_and_detach_keeps_session_alive() {
        let mut session = session();
        attach_both(&mut session);
        let consent = session.consent().clone();
        let edge = session.session().clone();

        let tap = invoke(3, "double-tap");
        let broadcasts = session
            .acknowledge_review_from_halo(&tap, Symbol::qualified("bridge", "packet-review"))
            .expect("Halo tap acknowledges the Viture review card");
        assert_broadcasts_to_both(&session, &broadcasts);
        let review = access::field(session.workspace().unwrap(), "review").unwrap();
        assert_eq!(access::field(review, "status"), Some(&build::sym("acked")));
        assert_eq!(
            access::field(review, "acked-by"),
            Some(&Expr::Symbol(session.surface(GlassesPeer::Halo).clone()))
        );
        assert_eq!(session.ledger().len(), 1);

        let removed = session.detach_viture();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].surface, *session.surface(GlassesPeer::Viture));
        assert_eq!(session.viture_loop_policy(), None);
        assert_eq!(session.live_bindings().len(), 1);
        assert_eq!(
            session.live_bindings()[0].surface,
            *session.surface(GlassesPeer::Halo)
        );
        assert_eq!(session.consent(), &consent);
        assert_eq!(session.session(), &edge);

        let halo_voice = edit_field(4, "status", build::sym("halo-only"));
        let broadcasts = session
            .submit_from(GlassesPeer::Halo, &halo_voice)
            .expect("Halo keeps editing after Viture detaches");
        assert_eq!(broadcasts.len(), 1);
        assert_eq!(broadcasts[0].surface, *session.surface(GlassesPeer::Halo));
        assert_eq!(session.ledger().len(), 2);

        let before_reattach_rows = session.ledger().len();
        let viture = SurfaceCaps::from_preset("glasses-luma-ultra", "viture.co-use").unwrap();
        sim_lib_scene::validate_scene(&session.attach_viture(viture).unwrap()).unwrap();
        assert_eq!(session.role(GlassesPeer::Viture), Some(SurfaceRole::Main));
        assert_eq!(session.live_bindings().len(), 2);
        assert_eq!(
            session.ledger().len(),
            before_reattach_rows,
            "reattach opens a projection but does not append an edit row"
        );
        assert_eq!(session.consent(), &consent);
    }

    fn assert_broadcasts_to_both(session: &GlassesCoUseSession, broadcasts: &[Broadcast]) {
        assert!(
            broadcasts
                .iter()
                .any(|broadcast| broadcast.surface == *session.surface(GlassesPeer::Viture)),
            "Viture did not receive a broadcast"
        );
        assert!(
            broadcasts
                .iter()
                .any(|broadcast| broadcast.surface == *session.surface(GlassesPeer::Halo)),
            "Halo did not receive a broadcast"
        );
    }
}
