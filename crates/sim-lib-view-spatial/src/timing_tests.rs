use std::rc::Rc;

use sim_kernel::Expr;
use sim_lib_scene::{Anchor, AnchorSpace, Transform3};
use sim_lib_view_device::{
    AdapterInput, DeviceSurfaceCapsExt, EncodedScene, GlanceInput, GlanceState,
};
use sim_value::{access, build};

use crate::{PoseView, clamp_predicted, halo_loop, viture_loop};

#[test]
fn reproject_loop_drops_and_clamps_stale_pose() {
    let encoded = EncodedScene::new(spatial_scene());
    let viture = sim_lib_view::SurfaceCaps::from_preset("glasses-luma-ultra", "viture")
        .unwrap()
        .device_profile();
    let (mut loop_, mut clock) = viture_loop(&viture, 12);
    assert_eq!(clock.rate.content_hz, 60);
    assert_eq!(clock.rate.adapt_hz, 120);
    assert_eq!(clock.rate.max_stale_ms, 25);
    assert_eq!(loop_.policy(), sim_lib_view_device::StalePolicy::Predict);
    let encode_count = 0;
    let mut dropped = 0;
    let mut sample_seq = 0;

    for batch in [3_u64, 3, 4] {
        let mut newest = PoseView::identity(sample_seq);
        for _ in 0..batch {
            sample_seq += 1;
            newest = PoseView::identity(sample_seq)
                .with_timing(1, 4_000_000)
                .with_angles(sample_seq as f64, 0.0, 0.0);
            loop_.offer(&newest);
        }
        let frame = loop_
            .step(
                &clock,
                &AdapterInput::new(encoded.clone(), 41, newest, clock.tick),
                &viture,
            )
            .expect("fresh reprojected frame");
        assert!(!frame.stale);
        assert_scene_kind(frame.out.as_ref(), "stereo");
        dropped += frame.dropped;
        clock.advance();
    }
    assert!(
        dropped >= 7,
        "expected at least 7 dropped poses, got {dropped}"
    );
    assert_eq!(encode_count, 0);

    clock.advance();
    let stale_pose = PoseView::identity(99)
        .with_timing(10, 40_000_000)
        .with_angles(25.0, 0.0, 0.0);
    assert_eq!(clamp_predicted(&stale_pose, 12).predict_ns, 12_000_000);
    loop_.offer(&stale_pose);
    let stale_predicted = loop_
        .step(
            &clock,
            &AdapterInput::new(encoded.clone(), 41, stale_pose, 0),
            &viture,
        )
        .expect("stale predicted frame");
    assert!(stale_predicted.stale);
    assert_scene_kind(stale_predicted.out.as_ref(), "stereo");
    assert_eq!(
        field_u64(stale_predicted.out.as_ref(), "predict-ms"),
        Some(12)
    );
    assert_eq!(
        field_u64(stale_predicted.out.as_ref(), "sample-seq"),
        Some(99)
    );

    let beyond_clamp = PoseView::identity(100)
        .with_timing(13, 80_000_000)
        .with_angles(80.0, 0.0, 0.0);
    loop_.offer(&beyond_clamp);
    let held = loop_
        .step(
            &clock,
            &AdapterInput::new(encoded, 41, beyond_clamp, 0),
            &viture,
        )
        .expect("held stale frame");
    assert!(held.stale);
    assert!(Rc::ptr_eq(&held.out, &stale_predicted.out));
    assert_eq!(encode_count, 0);
}

#[test]
fn halo_glance_loop_drops_and_holds_last() {
    let encoded = EncodedScene::new(sim_lib_scene::glance_card("Halo", None, None, "info", 1));
    let halo = sim_lib_view::SurfaceCaps::from_preset("glasses-hud", "halo")
        .unwrap()
        .device_profile();
    let (mut loop_, mut clock) = halo_loop(&halo);
    assert_eq!(clock.rate.content_hz, 5);
    assert_eq!(clock.rate.adapt_hz, 30);
    assert_eq!(loop_.policy(), sim_lib_view_device::StalePolicy::HoldLast);
    let encode_count = 0;
    let mut dropped = 0;
    let mut last_fresh = None;

    for tick in 0..3_u64 {
        let state = GlanceState::with_input(GlanceInput::Tap, tick + 1);
        for _ in 0..[3_u64, 3, 4][tick as usize] {
            loop_.offer(&state);
        }
        let frame = loop_
            .step(
                &clock,
                &AdapterInput::new(encoded.clone(), 72, state, clock.tick),
                &halo,
            )
            .expect("fresh halo frame");
        assert!(!frame.stale);
        assert_scene_kind(frame.out.as_ref(), "glance");
        dropped += frame.dropped;
        last_fresh = Some(frame.out);
        clock.advance();
    }

    assert!(
        dropped >= 7,
        "expected at least 7 dropped taps, got {dropped}"
    );
    assert_eq!(encode_count, 0);

    let held_from = last_fresh.expect("fresh frame");
    let stale_clock = sim_lib_view_device::FrameClock::new(7, halo.rate);
    let stale_state = GlanceState::with_input(GlanceInput::Tap, 99);
    loop_.offer(&stale_state);
    let stale = loop_
        .step(
            &stale_clock,
            &AdapterInput::new(encoded, 72, stale_state, 0),
            &halo,
        )
        .expect("stale halo frame");
    assert!(stale.stale);
    assert!(Rc::ptr_eq(&stale.out, &held_from));
    assert_eq!(encode_count, 0);
}

fn spatial_scene() -> Expr {
    sim_lib_scene::spatial(vec![sim_lib_scene::panel(
        "world",
        sim_lib_scene::node("text", vec![("text", build::text("World"))]),
        Anchor::new(AnchorSpace::World, "desk"),
        Transform3::new([0.0, 0.0, -1.6], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]),
    )])
}

fn assert_scene_kind(scene: &Expr, expected: &str) {
    let kind = sim_lib_scene::node_kind(scene).expect("scene kind");
    assert_eq!(kind.namespace.as_deref(), Some("scene"));
    assert_eq!(kind.name.as_ref(), expected);
}

fn field_u64(scene: &Expr, name: &str) -> Option<u64> {
    let Expr::Number(number) = access::field(scene, name)? else {
        return None;
    };
    number.canonical.parse().ok()
}
