//! DEVICE_3 adapter-loop wiring for the two glasses paths.

use sim_lib_view_device::{AdapterLoop, DeviceProfile, FrameClock, GlanceAdapter, StalePolicy};

use crate::{ClampedReprojector, halo_glance_config};

/// Viture loop type: stereo reprojection over the DEVICE_3 adapter loop.
pub type VitureReprojectLoop = AdapterLoop<ClampedReprojector>;

/// Halo loop type: the configured glance adapter over the DEVICE_3 adapter loop.
pub type HaloGlanceLoop = AdapterLoop<GlanceAdapter>;

/// Builds the Viture reprojection loop and modeled clock from its profile rate.
pub fn viture_loop(
    profile: &DeviceProfile,
    max_predict_ms: u64,
) -> (VitureReprojectLoop, FrameClock) {
    (
        AdapterLoop::new(
            ClampedReprojector::new(max_predict_ms),
            StalePolicy::Predict,
        ),
        FrameClock::at_zero(profile.rate),
    )
}

/// Builds the Halo glance loop and modeled clock from its profile rate.
pub fn halo_loop(profile: &DeviceProfile) -> (HaloGlanceLoop, FrameClock) {
    (
        AdapterLoop::new(halo_glance_config(), StalePolicy::HoldLast),
        FrameClock::at_zero(profile.rate),
    )
}
