//! Halo mapping through the shared device glance reducer.

use sim_kernel::{Expr, Result};
use sim_lib_view_device::{DeviceProfile, GlanceReducer};

/// Reduces a general Scene to the Halo mono-HUD Scene form.
///
/// This is intentionally a thin call into [`GlanceReducer`]. The shared device
/// layer owns card extraction and fitting semantics, so Halo stays a configured
/// one-card route instead of a private HUD encoder.
pub fn halo_glance_scene(scene: &Expr, profile: &DeviceProfile) -> Result<Expr> {
    GlanceReducer.reduce(scene, profile)
}
