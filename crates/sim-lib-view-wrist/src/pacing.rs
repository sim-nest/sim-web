//! Watch pacing configuration over the shared device adapter loop.

use sim_kernel::Result;
use sim_lib_stream_device::DeviceSample;
use sim_lib_view_device::{
    AdapterInput, AdapterLoop, DeviceProfile, EncodedScene, Frame, FrameClock, GlanceAdapter,
    GlanceState, StalePolicy,
};

use crate::watch_glance_adapter;

/// Builds a watch frame clock at tick zero from the profile's rate envelope.
pub fn watch_frame_clock(profile: &DeviceProfile) -> FrameClock {
    watch_frame_clock_at(profile, 0)
}

/// Builds a watch frame clock at `tick` from the profile's rate envelope.
pub fn watch_frame_clock_at(profile: &DeviceProfile, tick: u64) -> FrameClock {
    FrameClock::new(tick, profile.rate)
}

/// Builds the watch adapter loop with hold-last staleness behavior.
pub fn watch_adapter_loop(profile: &DeviceProfile) -> AdapterLoop<GlanceAdapter> {
    AdapterLoop::new(
        watch_glance_adapter(watch_profile_uses_large_face(profile)),
        StalePolicy::HoldLast,
    )
}

/// Converts the latest worn sample sequence into the glance adapter state.
pub fn worn_state_from<S: DeviceSample>(sample: &S) -> GlanceState {
    GlanceState::idle(sample.seq())
}

/// Records one available worn sample for the next adapter-loop step.
pub fn offer_worn<S: DeviceSample>(loop_: &mut AdapterLoop<GlanceAdapter>, sample: &S) {
    let state = worn_state_from(sample);
    loop_.offer(&state);
}

/// Advances the watch loop using the newest worn sample and an encoded glance card.
pub fn tick_worn<S: DeviceSample>(
    loop_: &mut AdapterLoop<GlanceAdapter>,
    clock: &FrameClock,
    encoded: &EncodedScene,
    encoded_seq: u64,
    newest: &S,
    profile: &DeviceProfile,
) -> Result<Frame> {
    let state = worn_state_from(newest);
    let input = AdapterInput::new(encoded.clone(), encoded_seq, state, newest.seq());
    loop_.step(clock, &input, profile)
}

fn watch_profile_uses_large_face(profile: &DeviceProfile) -> bool {
    matches!(
        profile.kind.name.as_ref(),
        "watch" | "watch-glance-large" | "watch-sport"
    )
}
