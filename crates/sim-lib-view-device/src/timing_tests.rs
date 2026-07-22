use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use sim_kernel::{Expr, Result};
use sim_value::build;

use crate::{
    AdapterInput, AdapterLoop, DeviceTier, EncodedScene, FrameClock, LocalAdapter, RateClass,
    StalePolicy, blank_frame, tier_preset,
};

#[test]
fn loop_counts_drops_and_honors_staleness() {
    let encoded = EncodedScene::new(encoded_scene());
    let profile = tier_preset(DeviceTier::Actuator);
    let mut clock = FrameClock::at_zero(RateClass::watch());
    let mut adapter_loop = AdapterLoop::new(StateTagAdapter, StalePolicy::Predict);
    let encode_count = Arc::new(AtomicUsize::new(0));
    let mut dropped = 0;

    for batch in [3_u64, 3, 4] {
        let mut newest = 0;
        for _ in 0..batch {
            newest += 1;
            adapter_loop.offer(&newest);
        }
        let input = AdapterInput::new(encoded.clone(), 11, newest, clock.tick);
        let frame = adapter_loop
            .step(&clock, &input, &profile)
            .expect("adapter step");
        dropped += frame.dropped;
        assert_eq!(frame.encoded_seq, 11);
        clock.advance();
    }

    assert!(
        dropped >= 7,
        "expected at least 7 dropped inputs, got {dropped}"
    );
    assert_eq!(encode_count.load(Ordering::SeqCst), 0);
}

#[test]
fn hold_last_repeats_previous_frame_for_stale_state() {
    let encoded = EncodedScene::new(encoded_scene());
    let profile = tier_preset(DeviceTier::Actuator);
    let rate = RateClass {
        content_hz: 1,
        adapt_hz: 1,
        max_stale_ms: 1,
    };
    let mut clock = FrameClock::at_zero(rate);
    let mut adapter_loop = AdapterLoop::new(StateTagAdapter, StalePolicy::HoldLast);

    adapter_loop.offer(&7);
    let first = adapter_loop
        .step(
            &clock,
            &AdapterInput::new(encoded.clone(), 3, 7, clock.tick),
            &profile,
        )
        .expect("fresh frame");
    assert!(!first.stale);

    clock.advance();
    clock.advance();
    adapter_loop.offer(&8);
    let stale = adapter_loop
        .step(&clock, &AdapterInput::new(encoded, 3, 8, 0), &profile)
        .expect("stale frame");

    assert!(stale.stale);
    assert!(Rc::ptr_eq(&stale.out, &first.out));
}

#[test]
fn blank_policy_emits_profile_tagged_blank_frame() {
    let encoded = EncodedScene::new(encoded_scene());
    let profile = tier_preset(DeviceTier::Display);
    let rate = RateClass {
        content_hz: 1,
        adapt_hz: 1,
        max_stale_ms: 1,
    };
    let clock = FrameClock::new(2, rate);
    let mut adapter_loop = AdapterLoop::new(StateTagAdapter, StalePolicy::Blank);

    adapter_loop.offer(&1);
    let frame = adapter_loop
        .step(&clock, &AdapterInput::new(encoded, 9, 1, 0), &profile)
        .expect("blank frame");

    assert!(frame.stale);
    assert_eq!(frame.out.as_ref(), &blank_frame(&profile));
}

#[test]
fn frame_clock_uses_modeled_ticks_for_staleness() {
    let rate = RateClass {
        content_hz: 1,
        adapt_hz: 10,
        max_stale_ms: 150,
    };
    let mut clock = FrameClock::at_zero(rate);
    clock.advance();
    assert_eq!(clock.elapsed_ms_since(0), 100);
    assert!(!clock.stale(0));
    clock.advance();
    assert_eq!(clock.elapsed_ms_since(0), 200);
    assert!(clock.stale(0));
}

#[derive(Clone, Copy, Debug)]
struct StateTagAdapter;

impl LocalAdapter for StateTagAdapter {
    type State = u64;

    fn adapt(
        &self,
        scene: &EncodedScene,
        state: &Self::State,
        _profile: &crate::DeviceProfile,
    ) -> Result<Rc<Expr>> {
        Ok(Rc::new(build::map(vec![
            ("kind", build::qsym("device", "adapted")),
            ("scene", scene.expr().clone()),
            ("state", build::uint(*state)),
        ])))
    }
}

fn encoded_scene() -> Expr {
    build::map(vec![
        ("kind", build::qsym("scene", "text")),
        ("text", build::text("encoded")),
    ])
}
