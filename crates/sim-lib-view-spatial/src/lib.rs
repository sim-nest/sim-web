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
pub mod layout;

pub use encode::{SPATIAL_SURFACE_CODEC_ID, SpatialSurfaceCodec, surface_spatial_codec_symbol};
pub use glance_map::halo_glance_scene;
pub use layout::{PanelLayout, SpatialLayout, arrange_spatial_panels};

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
