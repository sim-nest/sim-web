//! Reversible BRIDGE packet review surface for SIM Web.
//!
//! This is the human seat on a BRIDGE conversation. It renders one `BridgePacket`
//! (from `sim-codec-bridge`) as Scene data via `bridge_packet_view` and decodes
//! ordinary `intent/edit-field` values back into typed BRIDGE collaboration parts
//! via `bridge_packet_edit`. It is a view codec at the surface position, not a new
//! protocol.
//!
//! The point is symmetry: a human and an agent edit the *same* packet expression,
//! and a patch, review, vote, or receipt a person makes
//! (`patch_edit_intent` / `review_edit_intent` / `vote_edit_intent` /
//! `receipt_edit_intent`) is the identical BRIDGE part record a model produces.
//! So the merge policy in `sim-lib-bridge` cannot tell a human reviewer from a
//! model reviewer -- there is one shared object, not a browser copy beside a model
//! copy. `BridgePacketSurfaceCodec` / `BRIDGE_PACKET_SURFACE_CODEC_ID` register
//! the surface; `packet_review_demo` is the runnable cookbook seat.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod cookbook;
mod glasses_review;
mod surface;

pub use cookbook::packet_review_demo;
pub use glasses_review::{
    BRIDGE_WARRANT_REVIEW_MISSION_NAME, BRIDGE_WARRANT_REVIEW_MISSION_NAMESPACE,
    BridgeGlassesReviewInput, VITURE_WARRANT_REVIEW_PANEL_ID, WarrantReviewDecision,
    halo_warrant_glance_pager, viture_warrant_review_panel, viture_warrant_review_scene,
    warrant_review_intent, warrant_review_intent_from_glasses_input, warrant_review_mission,
};
pub use surface::{
    BRIDGE_PACKET_SURFACE_CODEC_ID, BridgePacketSurfaceCodec, bridge_packet_edit,
    bridge_packet_view, patch_edit_intent, receipt_edit_intent, review_edit_intent,
    vote_edit_intent,
};

#[cfg(test)]
mod glasses_tests;
#[cfg(test)]
mod tests;
