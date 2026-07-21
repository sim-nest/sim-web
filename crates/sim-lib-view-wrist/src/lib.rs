//! Wrist-specific glance budgets for SIM Web watch surfaces.
//!
//! The crate keeps the watch path as a thin configuration layer over
//! `sim-lib-view-device`: a round-screen budget, a haptic acknowledgement lane,
//! and constructors for the shared one-card device adapter.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod budget;
pub mod command;
pub mod pacing;
pub mod voice;

pub use budget::{
    WATCH_GLANCE_ACK_MS, WATCH_GLANCE_CELLS, WATCH_GLANCE_GLYPHS, WATCH_GLANCE_LARGE_CELLS,
    WATCH_GLANCE_LARGE_GLYPHS, watch_glance_adapter, watch_glance_budget, watch_glance_budget_demo,
    watch_glance_large_adapter, watch_glance_large_budget,
};
pub use command::{HapticPattern, HapticStep, Urgency, WatchCommand};
pub use pacing::{
    offer_worn, tick_worn, watch_adapter_loop, watch_frame_clock, watch_frame_clock_at,
    worn_state_from,
};
pub use voice::{
    AudioFrame, CAP_WATCH_MIC, MAX_MIC_FRAME_BYTES, MAX_MIC_FRAMES, MicCapture,
    transcribe_via_site, watch_mic_capability, watch_mic_grant,
};

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod budget_tests;

#[cfg(test)]
mod command_tests;

#[cfg(test)]
mod pacing_tests;

#[cfg(test)]
mod voice_tests;
