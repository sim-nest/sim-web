use std::rc::Rc;

use sim_kernel::Expr;
use sim_lib_scene::{Anchor, AnchorSpace, Transform3};
use sim_lib_view_device::{
    AckChannel, DeviceSurfaceCapsExt, EncodedScene, GlanceInput, GlanceState, LocalAdapter,
};
use sim_value::{access, build};

use crate::{PoseView, Reprojector, halo_glance_config};

#[test]
fn reprojector_bespoke_halo_uses_glance_adapter() {
    let encoded_spatial = EncodedScene::new(spatial_scene());
    let viture = sim_lib_view::SurfaceCaps::from_preset("glasses-luma-ultra", "viture")
        .unwrap()
        .device_profile();
    let reprojector = Reprojector::new(12);
    let encode_count = 1;

    let first = reprojector
        .adapt(
            &encoded_spatial,
            &PoseView::identity(1)
                .with_timing(1, 40_000_000)
                .with_translation([0.1, 0.0, 0.0])
                .with_angles(8.0, 0.0, 0.0),
            &viture,
        )
        .unwrap();
    let second = reprojector
        .adapt(
            &encoded_spatial,
            &PoseView::identity(2)
                .with_timing(2, 4_000_000)
                .with_translation([0.0, 0.0, 0.0]),
            &viture,
        )
        .unwrap();
    assert_scene_kind(first.as_ref(), "stereo");
    assert_scene_kind(second.as_ref(), "stereo");
    assert_ne!(first, second);
    assert_eq!(stereo_children(first.as_ref(), "left-eye").len(), 2);
    assert_eq!(stereo_children(first.as_ref(), "right-eye").len(), 2);
    assert_anchor_rule(first.as_ref(), "world-locked");
    assert_anchor_rule(first.as_ref(), "head-locked");
    assert_eq!(encode_count, 1);

    let mirror_profile = sim_lib_view::SurfaceCaps::from_preset("glasses-stereo", "mirror")
        .unwrap()
        .device_profile();
    let mirrored = reprojector
        .adapt(&encoded_spatial, &PoseView::identity(3), &mirror_profile)
        .unwrap();
    assert!(Rc::ptr_eq(&mirrored, &encoded_spatial.shared()));

    let halo = halo_glance_config();
    assert_eq!(halo.budget.ack, AckChannel::GlyphFlash);
    assert_eq!(halo.budget.cells, 6);
    assert_eq!(halo.budget.glyphs, 128);

    let encoded_glance =
        EncodedScene::new(sim_lib_scene::glance_card("Halo", None, None, "info", 1));
    let halo_profile = sim_lib_view::SurfaceCaps::from_preset("glasses-hud", "halo")
        .unwrap()
        .device_profile();
    let tapped = halo
        .adapt(
            &encoded_glance,
            &GlanceState::with_input(GlanceInput::Tap, 7),
            &halo_profile,
        )
        .unwrap();
    let tapped_again = halo
        .adapt(
            &encoded_glance,
            &GlanceState::with_input(GlanceInput::Tap, 8),
            &halo_profile,
        )
        .unwrap();
    assert_scene_kind(tapped.as_ref(), "glance");
    assert_scene_kind(tapped_again.as_ref(), "glance");
    assert_ne!(tapped, tapped_again);
    assert_eq!(
        access::field_sym(tapped.as_ref(), "ack-channel")
            .unwrap()
            .name
            .as_ref(),
        "glyph-flash"
    );
    assert_eq!(
        access::field_sym(tapped.as_ref(), "ack-input")
            .unwrap()
            .name
            .as_ref(),
        "tap"
    );
    assert_eq!(field_u64(tapped.as_ref(), "ack-ms"), Some(120));
    assert_eq!(field_u64(tapped.as_ref(), "ack-tick"), Some(7));
    assert_eq!(field_u64(tapped_again.as_ref(), "ack-tick"), Some(8));
    assert_eq!(encode_count, 1);
}

fn spatial_scene() -> Expr {
    sim_lib_scene::spatial(vec![
        sim_lib_scene::panel(
            "world",
            sim_lib_scene::node("text", vec![("text", build::text("World"))]),
            Anchor::new(AnchorSpace::World, "desk"),
            Transform3::new([0.0, 0.0, -1.6], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]),
        ),
        sim_lib_scene::panel(
            "head",
            sim_lib_scene::node("text", vec![("text", build::text("Head"))]),
            Anchor::new(AnchorSpace::Head, "view"),
            Transform3::identity(),
        ),
        sim_lib_scene::panel(
            "culled",
            sim_lib_scene::node("text", vec![("text", build::text("Hidden"))]),
            Anchor::new(AnchorSpace::World, "side"),
            Transform3::new([5.0, 0.0, -1.0], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]),
        ),
    ])
}

fn assert_scene_kind(scene: &Expr, expected: &str) {
    let kind = sim_lib_scene::node_kind(scene).expect("scene kind");
    assert_eq!(kind.namespace.as_deref(), Some("scene"));
    assert_eq!(kind.name.as_ref(), expected);
}

fn stereo_children<'a>(scene: &'a Expr, eye_field: &str) -> &'a [Expr] {
    let eye = access::field(scene, eye_field).expect("eye field");
    match access::field(eye, "children").expect("children") {
        Expr::List(children) => children,
        other => panic!("children must be list, got {other:?}"),
    }
}

fn assert_anchor_rule(scene: &Expr, expected: &str) {
    let found = ["left-eye", "right-eye"].into_iter().any(|eye| {
        stereo_children(scene, eye).iter().any(|child| {
            access::field_sym(child, "anchor-rule")
                .is_some_and(|symbol| symbol.name.as_ref() == expected)
        })
    });
    assert!(found, "missing anchor rule {expected}");
}

fn field_u64(scene: &Expr, name: &str) -> Option<u64> {
    let Expr::Number(number) = access::field(scene, name)? else {
        return None;
    };
    number.canonical.parse().ok()
}
