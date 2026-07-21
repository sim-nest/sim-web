//! Spatial glasses surface encoding for SIM Web.
//!
//! This crate adds `surface:spatial`, a [`sim_lib_view::SurfaceCodec`] that
//! keeps content-rate packets independent of device tracking samples. It renders
//! values through the universal Scene view, then selects one of the glasses
//! class routes supplied by [`sim_lib_view_device`]: pose-free spatial panels for
//! stereo 6DoF displays, the shared one-card glance reducer for mono HUDs, or a
//! reduced mirrored Scene for display-only glasses.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod encode;
pub mod glance_map;
pub mod halo_glance;
pub mod layout;
pub mod pose_view;
pub mod predict;
pub mod rank;
pub mod reproject;
pub mod reproject_loop;

pub use encode::{SPATIAL_SURFACE_CODEC_ID, SpatialSurfaceCodec, surface_spatial_codec_symbol};
pub use glance_map::halo_glance_scene;
pub use halo_glance::{HALO_ACK_MS, halo_glance_budget, halo_glance_config};
pub use layout::{PanelLayout, SpatialLayout, arrange_spatial_panels};
pub use pose_view::PoseView;
pub use predict::{ClampedReprojector, clamp_predicted};
pub use rank::{AttentionBudget, rank_for_profile, rank_glasses, rank_spatial};
pub use reproject::Reprojector;
pub use reproject_loop::{HaloGlanceLoop, VitureReprojectLoop, halo_loop, viture_loop};

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod adapter_tests;
#[cfg(test)]
mod rank_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod timing_tests;
