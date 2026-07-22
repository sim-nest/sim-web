//! Local adapter for one-card glance surfaces.

use std::rc::Rc;

use sim_kernel::{Error, Expr, Result};
use sim_value::build;

use crate::{
    DeviceProfile, EncodedScene, GlanceBudget, GlanceInput, GlanceState, LocalAdapter,
    fit_to_budget,
};

/// The shared one-card adapter for every "one card plus one ack" tier.
///
/// Constraint: a device at or below the "one card plus one ack" tier MUST
/// configure this adapter rather than write its own card reducer. The HUD
/// glasses tier and watch glance tier share this single type, differing only by
/// [`GlanceBudget`] and [`crate::AckChannel`].
///
/// Soften path: a bespoke [`LocalAdapter`] is reserved for surfaces that are
/// genuinely richer than a glance, and the adapter introduction must document
/// the architectural reason. To relax the rule globally, extend
/// [`GlanceBudget`] with the new capability so one-card tiers stay unified;
/// drop to a bespoke adapter only after that extension cannot express the need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlanceAdapter {
    /// Local fitting budget.
    pub budget: GlanceBudget,
    /// Ack duration in modeled milliseconds.
    pub ack_ms: u64,
}

impl GlanceAdapter {
    /// Builds an adapter for one glance budget.
    pub fn new(budget: GlanceBudget, ack_ms: u64) -> Self {
        Self { budget, ack_ms }
    }
}

impl LocalAdapter for GlanceAdapter {
    type State = GlanceState;

    fn adapt(
        &self,
        scene: &EncodedScene,
        state: &Self::State,
        _profile: &DeviceProfile,
    ) -> Result<Rc<Expr>> {
        if !crate::glance::is_glance(scene.expr()) {
            return Err(Error::HostError(
                "GlanceAdapter expects an encoded scene/glance card".to_owned(),
            ));
        }
        let card = fit_to_budget(scene.expr(), &self.budget)?;
        Ok(Rc::new(match state.pending_input {
            Some(input) => with_ack(card, input, state.tick, self.budget, self.ack_ms),
            None => card,
        }))
    }
}

fn with_ack(card: Expr, input: GlanceInput, tick: u64, budget: GlanceBudget, ack_ms: u64) -> Expr {
    sim_value::access::set(
        &sim_value::access::set(
            &sim_value::access::set(
                &sim_value::access::set(&card, "ack-channel", Expr::Symbol(budget.ack.to_symbol())),
                "ack-input",
                build::sym(input.token()),
            ),
            "ack-ms",
            build::uint(ack_ms),
        ),
        "ack-tick",
        build::uint(tick),
    )
}
