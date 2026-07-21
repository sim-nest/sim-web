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

pub mod consent;
pub mod encode;
pub mod glance_map;
pub mod halo_glance;
pub mod layout;
pub mod pose_view;
pub mod predict;
pub mod rank;
pub mod reproject;
pub mod reproject_loop;
pub mod voice_site;
pub mod world;

pub use consent::{
    CAP_GLASSES_CAMERA, CAP_GLASSES_HAND, CAP_GLASSES_MIC, CAP_GLASSES_POSE,
    CAP_GLASSES_VENDOR_REPORT, CAP_GLASSES_WORLD_ANCHOR, GlassesCapability,
    active_glasses_consent_badge_cluster, glasses_camera_grant, glasses_capability_for_expr,
    glasses_hand_grant, glasses_mic_capability, glasses_mic_grant, glasses_pose_grant,
    glasses_vendor_report_grant, glasses_world_anchor_grant, halo_consent_glyph,
    require_glasses_consent, require_glasses_expr_consent, store_glasses_sample,
    sweep_glasses_privacy,
};
pub use encode::{SPATIAL_SURFACE_CODEC_ID, SpatialSurfaceCodec, surface_spatial_codec_symbol};
pub use glance_map::halo_glance_scene;
pub use halo_glance::{HALO_ACK_MS, halo_glance_budget, halo_glance_config};
pub use layout::{
    GlancePreference, PanelLayout, PanelPlacement, SpatialLayout, WORKSPACE_LAYOUT_KIND,
    WORKSPACE_LAYOUT_NAMESPACE, WORKSPACE_LAYOUT_TABLE_NAMESPACE, WorkspaceLayout,
    arrange_spatial_panels, layout_load_op, layout_save_op, layout_table_key,
};
pub use pose_view::PoseView;
pub use predict::{ClampedReprojector, clamp_predicted};
pub use rank::{AttentionBudget, rank_for_profile, rank_glasses, rank_spatial};
pub use reproject::Reprojector;
pub use reproject_loop::{HaloGlanceLoop, VitureReprojectLoop, halo_loop, viture_loop};
pub use voice_site::{
    AsrSite, AsrSitePlacement, XR_MIC_CHUNK_KIND, XR_MIC_CHUNK_NAMESPACE, XrMicChunkRef,
    voice_intent_via_site,
};
pub use world::{
    AnchorResolution, VioTrackingStatus, WORLD_ANCHOR_REASON_NAMESPACE, WorldAnchorObservation,
    WorldAnchorResolver, XR_TRACKING_STATUS_NAMESPACE, resolve_world_anchor,
};

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod adapter_tests;
#[cfg(test)]
mod consent_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod rank_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod timing_tests;
#[cfg(test)]
mod voice_site_tests;
#[cfg(test)]
mod world_tests;
