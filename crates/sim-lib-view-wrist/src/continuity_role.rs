//! Expiring optional-endpoint roles for continuity projections.
//!
//! This adapter deliberately owns neither continuity nor product state. It
//! projects one caller-filtered glance and translates a very small semantic
//! action vocabulary while the caller's authority evidence remains current.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_intent::{Origin, intent};
use sim_lib_scene::GlanceCard;

/// Roles an optional endpoint may supply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointRole {
    /// Display one already-filtered `scene/glance`.
    Glance,
    /// Emit the bounded continuity action vocabulary.
    SemanticAction,
}

/// Provider-described candidate, intentionally free of brand classifications.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndpointCandidate {
    /// Provider-local endpoint identity, used only for availability evidence.
    pub id: Symbol,
    /// Narrow roles supported by this candidate.
    pub roles: Vec<EndpointRole>,
}

impl EndpointCandidate {
    /// Returns whether this candidate supplies both optional endpoint roles.
    pub fn is_admissible(&self) -> bool {
        self.roles.contains(&EndpointRole::Glance)
            && self.roles.contains(&EndpointRole::SemanticAction)
    }
}

/// Complete, caller-owned authority evidence for one event boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleAuthority {
    /// The provider still reports the endpoint connected.
    pub connected: bool,
    /// Current session consent covers this endpoint.
    pub session_consent: bool,
    /// The continuity surface has visible focus.
    pub visible_focus: bool,
    /// Product policy currently admits the narrow role.
    pub role_admitted: bool,
    /// Fresh provider evidence backs the endpoint.
    pub provider_evidence: bool,
    /// The matching route is still present.
    pub route_present: bool,
    /// The continuity root is still present.
    pub root_present: bool,
    /// Generation of the matching route lease.
    pub route_generation: u64,
    /// Exclusive monotonic expiry of the matching route lease.
    pub route_expires_at_ms: u64,
    /// Current monotonic time.
    pub now_ms: u64,
}

impl RoleAuthority {
    fn is_live(self) -> bool {
        self.connected
            && self.session_consent
            && self.visible_focus
            && self.role_admitted
            && self.provider_evidence
            && self.route_present
            && self.root_present
            && self.now_ms < self.route_expires_at_ms
    }
}

/// The only events accepted from an optional endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointAction {
    /// Invoke the glance's one primary semantic action.
    Primary,
    /// Acknowledge the current turn.
    Acknowledge,
    /// Defer the current turn.
    Defer,
    /// Cancel the current turn.
    Cancel,
    /// Stop current and pending activity.
    Stop,
}

/// Disposable projection returned to an endpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct EndpointProjection {
    /// The single already-filtered glance Scene.
    pub glance: Expr,
    /// Route generation that fences every event from this projection.
    pub generation: u64,
    primary_target: Option<Expr>,
}

/// Stateless-content adapter with only an expiring generation fence.
#[derive(Clone, Debug, Default)]
pub struct OptionalEndpointRoleAdapter {
    live_generation: Option<u64>,
}

impl OptionalEndpointRoleAdapter {
    /// Projects one filtered glance while all role authority is live.
    ///
    /// The adapter retains only the generation fence: never the Scene,
    /// transcript, audio, result, private payload, model memory, or product
    /// truth.
    pub fn project(
        &mut self,
        candidate: &EndpointCandidate,
        glance: &Expr,
        authority: RoleAuthority,
    ) -> Result<EndpointProjection> {
        if !candidate.is_admissible() || !authority.is_live() {
            self.invalidate();
            return Err(refused("optional endpoint role is not currently admitted"));
        }
        let card = GlanceCard::from_scene(glance)?;
        self.live_generation = Some(authority.route_generation);
        Ok(EndpointProjection {
            glance: glance.clone(),
            generation: authority.route_generation,
            primary_target: card.action.map(|action| action.target),
        })
    }

    /// Reduces a bounded endpoint event to an ordinary semantic Intent.
    pub fn intent_for(
        &mut self,
        projection: &EndpointProjection,
        action: EndpointAction,
        authority: RoleAuthority,
        logical_time: u64,
    ) -> Result<Expr> {
        if !authority.is_live()
            || self.live_generation != Some(projection.generation)
            || authority.route_generation != projection.generation
        {
            self.invalidate();
            return Err(refused("stale or unauthorized optional endpoint event"));
        }

        let origin = Origin::human(logical_time);
        let target = Expr::Symbol(Symbol::qualified("surface", "continuity"));
        match action {
            EndpointAction::Primary => {
                let primary = projection
                    .primary_target
                    .clone()
                    .ok_or_else(|| refused("glance has no primary semantic action"))?;
                Ok(invoke(origin, target, "primary", vec![primary]))
            }
            EndpointAction::Acknowledge => Ok(invoke(origin, target, "acknowledge", vec![])),
            EndpointAction::Defer => Ok(invoke(origin, target, "defer", vec![])),
            EndpointAction::Cancel => Ok(intent(
                "cancel",
                origin,
                vec![("pane", Expr::String("continuity".to_owned()))],
            )),
            EndpointAction::Stop => {
                let out = invoke(origin, target, "stop", vec![]);
                self.invalidate();
                Ok(out)
            }
        }
    }

    /// Invalidates every projection from the current and prior generations.
    pub fn invalidate(&mut self) {
        self.live_generation = None;
    }
}

fn invoke(origin: Origin, target: Expr, operation: &str, args: Vec<Expr>) -> Expr {
    intent(
        "invoke",
        origin,
        vec![
            ("target", target),
            (
                "op",
                Expr::Symbol(Symbol::qualified("continuity/action", operation)),
            ),
            ("args", Expr::List(args)),
        ],
    )
}

fn refused(message: &str) -> Error {
    Error::HostError(message.to_owned())
}
