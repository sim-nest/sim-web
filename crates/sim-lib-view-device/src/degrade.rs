//! Degradation from requested profile to observed route capabilities.

use crate::ladder::DeviceTier;
use crate::profile::{DeviceProfile, DeviceProfileParts, derive_tier, has_symbol, push_existing};
use sim_kernel::{Expr, Symbol};

/// Capabilities observed on a concrete route and its attached accessories.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObservedRoute {
    /// Observed display capability tokens.
    pub display: Vec<Symbol>,
    /// Observed input capability tokens.
    pub input: Vec<Symbol>,
    /// Observed output capability tokens.
    pub output: Vec<Symbol>,
    /// Observed link tokens.
    pub links: Vec<Symbol>,
    /// Observed stream tokens.
    pub streams: Vec<Symbol>,
}

impl ObservedRoute {
    /// Builds an observed route from an existing profile.
    pub fn from_profile(profile: &DeviceProfile) -> Self {
        Self {
            display: profile.display.clone(),
            input: profile.input.clone(),
            output: profile.output.clone(),
            links: profile.links.clone(),
            streams: profile.streams.clone(),
        }
    }
}

/// The resolved device tier and any unavailable requested capabilities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Degradation {
    /// Highest tier supported by the observed route.
    pub tier: DeviceTier,
    /// Human-readable explanations for unavailable requested capabilities.
    pub reasons: Vec<String>,
}

/// Resolves device degradation against observed route/accessory data.
#[derive(Clone, Debug, Default)]
pub struct DegradationResolver;

impl DegradationResolver {
    /// Maps the requested profile to the highest observed tier with reasons for
    /// every unavailable requested capability.
    pub fn resolve(requested: &DeviceProfile, observed: &ObservedRoute) -> Degradation {
        let supported = DeviceProfile::new(DeviceProfileParts {
            kind: requested.kind.clone(),
            display: intersection(&requested.display, &observed.display),
            input: intersection(&requested.input, &observed.input),
            output: intersection(&requested.output, &observed.output),
            links: intersection(&requested.links, &observed.links),
            streams: intersection(&requested.streams, &observed.streams),
            rate: requested.rate,
            policy: Expr::Map(Vec::new()),
        });
        let mut reasons = Vec::new();
        missing_reasons(
            &mut reasons,
            "display",
            &requested.display,
            &observed.display,
        );
        missing_reasons(&mut reasons, "input", &requested.input, &observed.input);
        missing_reasons(&mut reasons, "output", &requested.output, &observed.output);
        missing_reasons(&mut reasons, "link", &requested.links, &observed.links);
        missing_reasons(
            &mut reasons,
            "stream",
            &requested.streams,
            &observed.streams,
        );
        Degradation {
            tier: derive_tier(&supported),
            reasons,
        }
    }

    /// Returns a display-only degradation result with a single explanation.
    pub fn unavailable(reason: impl Into<String>) -> Degradation {
        Degradation {
            tier: DeviceTier::Display,
            reasons: vec![reason.into()],
        }
    }
}

fn intersection(requested: &[Symbol], observed: &[Symbol]) -> Vec<Symbol> {
    let mut out = Vec::new();
    for symbol in requested {
        if observed.iter().any(|candidate| candidate == symbol) {
            push_existing(&mut out, symbol.clone());
        }
    }
    out
}

fn missing_reasons(
    reasons: &mut Vec<String>,
    lane: &str,
    requested: &[Symbol],
    observed: &[Symbol],
) {
    for symbol in requested {
        if !has_symbol(observed, symbol.name.as_ref()) {
            reasons.push(format!("missing {lane}: {}", symbol.as_qualified_str()));
        }
    }
}
