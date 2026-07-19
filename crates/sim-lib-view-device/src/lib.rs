//! Device profiles and timing loops for SIM Web surface capabilities.
//!
//! Surfaces still advertise open [`sim_lib_view::SurfaceCaps`] metadata. This
//! crate adds the typed device view over that metadata: a timing envelope,
//! an ordered tier ladder, a single tier derivation function, a deterministic
//! [`FrameClock`] and [`AdapterLoop`], and degradation reasons when an observed
//! route cannot provide every requested capability. It remains library-level
//! surface logic; the kernel does not learn a device enum.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod adapter;
pub mod clock;
pub mod degrade;
pub mod glance;
pub mod glance_adapter;
pub mod ladder;
pub mod r#loop;
pub mod profile;
pub mod rate;
pub mod split;

pub use adapter::{EncodedScene, LocalAdapter, MirrorAdapter};
pub use clock::FrameClock;
pub use degrade::{Degradation, DegradationResolver, ObservedRoute};
pub use glance::{
    AckChannel, GlanceBudget, GlanceInput, GlanceReducer, GlanceState, fit_to_budget,
    reduce_scene_to_glance,
};
pub use glance_adapter::GlanceAdapter;
pub use ladder::DeviceTier;
pub use r#loop::{AdapterInput, AdapterLoop, Frame, StalePolicy, blank_frame};
pub use profile::{
    DEVICE_PROFILE_KIND, DEVICE_PROFILE_NAMESPACE, DeviceProfile, DeviceProfileError,
    DeviceProfileParts, DeviceSurfaceCapsExt, derive_tier, device_profile_demo, tier_preset,
};
pub use rate::{RateClass, RateError};
pub use split::{Split, SplitRun, drive};

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod profile_tests;

#[cfg(test)]
mod split_tests;

#[cfg(test)]
mod timing_tests;

#[cfg(test)]
mod glance_tests;
