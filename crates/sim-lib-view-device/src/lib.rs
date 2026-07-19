//! Device profile envelopes for SIM Web surface capabilities.
//!
//! Surfaces still advertise open [`sim_lib_view::SurfaceCaps`] metadata. This
//! crate adds the typed device view over that metadata: a timing envelope,
//! an ordered tier ladder, a single tier derivation function, and degradation
//! reasons when an observed route cannot provide every requested capability.
//! It remains library-level surface logic; the kernel does not learn a device
//! enum.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod degrade;
pub mod ladder;
pub mod profile;
pub mod rate;

pub use degrade::{Degradation, DegradationResolver, ObservedRoute};
pub use ladder::DeviceTier;
pub use profile::{
    DEVICE_PROFILE_KIND, DEVICE_PROFILE_NAMESPACE, DeviceProfile, DeviceProfileError,
    DeviceProfileParts, DeviceSurfaceCapsExt, derive_tier, device_profile_demo, tier_preset,
};
pub use rate::{RateClass, RateError};

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod profile_tests;
