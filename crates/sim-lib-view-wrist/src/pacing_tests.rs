use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::GlanceCard;
use sim_lib_stream_device::{DeviceSample, DeviceSampleError, DeviceSampleResult};
use sim_lib_view_device::{DeviceTier, EncodedScene, RateClass, StalePolicy, tier_preset};
use sim_value::{access, build};

use crate::{
    WATCH_GLANCE_CELLS, WATCH_GLANCE_LARGE_CELLS, offer_worn, tick_worn, watch_adapter_loop,
    watch_frame_clock, watch_frame_clock_at, worn_state_from,
};

#[test]
fn watch_loop_uses_profile_rate_hold_last_and_round_budget() {
    let mut profile = tier_preset(DeviceTier::Actuator);
    profile.kind = Symbol::new("watch-glance");

    let clock = watch_frame_clock(&profile);
    assert_eq!(clock.rate, RateClass::watch());

    let adapter_loop = watch_adapter_loop(&profile);
    assert_eq!(adapter_loop.policy(), StalePolicy::HoldLast);
    assert_eq!(adapter_loop.adapter().budget.cells, WATCH_GLANCE_CELLS);

    let mut large = profile.clone();
    large.kind = Symbol::new("watch-glance-large");
    let large_loop = watch_adapter_loop(&large);
    assert_eq!(large_loop.adapter().budget.cells, WATCH_GLANCE_LARGE_CELLS);
}

#[test]
fn worn_state_uses_sample_seq() {
    let sample = TestWornEvent::new(42);
    let state = worn_state_from(&sample);
    assert_eq!(state.tick, 42);
    assert!(state.pending_input.is_none());
}

#[test]
fn stale_worn_sample_holds_last_card_and_counts_drops() {
    let profile = wrist_profile();
    let encoded = EncodedScene::new(glance_scene("Wrist status"));
    let encoded_seq = 17;
    let encode_count = Arc::new(AtomicUsize::new(0));
    let mut adapter_loop = watch_adapter_loop(&profile);
    let fresh = TestWornEvent::new(0);

    offer_worn(&mut adapter_loop, &fresh);
    let first = tick_worn(
        &mut adapter_loop,
        &watch_frame_clock_at(&profile, 0),
        &encoded,
        encoded_seq,
        &fresh,
        &profile,
    )
    .expect("fresh frame");
    assert!(!first.stale);
    assert_eq!(first.dropped, 0);

    for _ in 0..3 {
        offer_worn(&mut adapter_loop, &fresh);
    }
    let stale = tick_worn(
        &mut adapter_loop,
        &watch_frame_clock_at(&profile, 5),
        &encoded,
        encoded_seq,
        &fresh,
        &profile,
    )
    .expect("stale frame");

    assert!(stale.stale);
    assert_eq!(stale.dropped, 2);
    assert!(std::rc::Rc::ptr_eq(&stale.out, &first.out));
    assert_eq!(field_u64(stale.out.as_ref(), "cells"), Some(3));
    assert_eq!(stale.encoded_seq, encoded_seq);
    assert_eq!(encode_count.load(Ordering::SeqCst), 0);
}

#[test]
fn offered_worn_samples_coalesce_between_two_ticks() {
    let profile = wrist_profile();
    let encoded = EncodedScene::new(glance_scene("Pacing"));
    let mut adapter_loop = watch_adapter_loop(&profile);
    let mut dropped = 0;
    let encode_count = Arc::new(AtomicUsize::new(0));

    for seq in 0..5 {
        offer_worn(&mut adapter_loop, &TestWornEvent::new(seq));
    }
    dropped += tick_worn(
        &mut adapter_loop,
        &watch_frame_clock_at(&profile, 4),
        &encoded,
        21,
        &TestWornEvent::new(4),
        &profile,
    )
    .expect("first tick")
    .dropped;

    for seq in 5..10 {
        offer_worn(&mut adapter_loop, &TestWornEvent::new(seq));
    }
    dropped += tick_worn(
        &mut adapter_loop,
        &watch_frame_clock_at(&profile, 9),
        &encoded,
        21,
        &TestWornEvent::new(9),
        &profile,
    )
    .expect("second tick")
    .dropped;

    assert!(
        dropped >= 8,
        "expected at least 8 coalesced worn samples, got {dropped}"
    );
    assert_eq!(encode_count.load(Ordering::SeqCst), 0);
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestWornEvent {
    seq: u64,
}

impl TestWornEvent {
    fn new(seq: u64) -> Self {
        Self { seq }
    }
}

impl DeviceSample for TestWornEvent {
    fn sample_kind() -> &'static str {
        "test-worn-event"
    }

    fn seq(&self) -> u64 {
        self.seq
    }

    fn to_expr(&self) -> Expr {
        build::map(vec![
            ("kind", build::qsym("test", "worn-event")),
            ("seq", build::uint(self.seq)),
        ])
    }

    fn from_expr(expr: &Expr) -> DeviceSampleResult<Self> {
        let seq = field_u64(expr, "seq")
            .ok_or_else(|| DeviceSampleError::new("test worn event missing seq"))?;
        Ok(Self::new(seq))
    }
}

fn wrist_profile() -> sim_lib_view_device::DeviceProfile {
    let mut profile = tier_preset(DeviceTier::Actuator);
    profile.kind = Symbol::new("watch-glance");
    profile.rate = RateClass::watch();
    profile
}

fn glance_scene(title: &str) -> Expr {
    GlanceCard::new(title, None, None, "normal", 1).to_scene()
}

fn field_u64(expr: &Expr, field: &str) -> Option<u64> {
    let Expr::Number(number) = access::field(expr, field)? else {
        return None;
    };
    number.canonical.parse().ok()
}
