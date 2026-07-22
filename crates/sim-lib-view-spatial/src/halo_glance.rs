//! Halo configuration over the shared DEVICE_3 glance adapter.

use sim_lib_view_device::{AckChannel, GlanceAdapter, GlanceBudget};

/// Modeled acknowledgement duration for a Halo tap.
pub const HALO_ACK_MS: u64 = 120;

/// Returns the Halo mono-HUD one-card budget.
pub fn halo_glance_budget() -> GlanceBudget {
    GlanceBudget {
        cells: 6,
        glyphs: 128,
        ack: AckChannel::GlyphFlash,
    }
}

/// Builds the shared DEVICE_3 [`GlanceAdapter`] configured for Halo.
///
/// Halo remains a one-card device path. This function supplies only a budget and
/// acknowledgement duration; card extraction and tap acknowledgement stay in the
/// shared device adapter.
pub fn halo_glance_config() -> GlanceAdapter {
    GlanceAdapter::new(halo_glance_budget(), HALO_ACK_MS)
}
