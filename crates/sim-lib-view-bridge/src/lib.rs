//! Reversible BRIDGE packet review surface for SIM Web.
//!
//! The surface renders one `BridgePacket` as Scene data and decodes ordinary
//! `intent/edit-field` values into typed BRIDGE collaboration parts. Human and
//! agent operators edit the same packet expression: a patch, review, vote, or
//! receipt is a BRIDGE part record, not a separate browser-side protocol.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod cookbook;
mod surface;

pub use cookbook::packet_review_demo;
pub use surface::{
    BRIDGE_PACKET_SURFACE_CODEC_ID, BridgePacketSurfaceCodec, bridge_packet_edit,
    bridge_packet_view, patch_edit_intent, receipt_edit_intent, review_edit_intent,
    vote_edit_intent,
};

#[cfg(test)]
mod tests;
