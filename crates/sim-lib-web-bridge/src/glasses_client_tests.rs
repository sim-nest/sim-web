use sim_kernel::Expr;
use sim_lib_scene::{Anchor, AnchorSpace, GlanceCard, GlanceMetric, Transform3};
use sim_lib_view::SurfaceCaps;
use sim_lib_view_device::GlanceState;
use sim_lib_view_spatial::PoseView;
use sim_value::{access, build};

use crate::{HaloPreviewClient, VitureSceneClient};

#[test]
fn spatial_reprojects_and_glance_renders() {
    let caps = SurfaceCaps::from_preset("glasses-luma-ultra", "viture.browser").unwrap();
    let mut viture = VitureSceneClient::new(&caps, 12).unwrap();
    viture.receive(spatial_scene()).unwrap();

    let first = viture
        .frame(
            &PoseView::identity(1)
                .with_timing(1, 40_000_000)
                .with_translation([0.2, 0.0, 0.0])
                .with_angles(10.0, 0.0, 0.0),
        )
        .unwrap();
    let second = viture
        .frame(
            &PoseView::identity(2)
                .with_timing(2, 4_000_000)
                .with_translation([0.0, 0.0, 0.0]),
        )
        .unwrap();

    assert_eq!(viture.content_receipts(), 1);
    assert_eq!(viture.viewport().per_eye_px(), [1920, 1200]);
    assert_eq!(viture.viewport().frame_px(), [3840, 1200]);
    assert_scene_kind(first.as_ref(), "stereo");
    assert_scene_kind(second.as_ref(), "stereo");
    assert_eq!(field_u64(first.as_ref(), "predict-ms"), Some(12));
    assert_eq!(field_pair(first.as_ref(), "eye-px"), Some([1920, 1200]));
    assert_eq!(field_pair(first.as_ref(), "frame-px"), Some([3840, 1200]));
    assert_eq!(eye_children(first.as_ref(), "left-eye").len(), 1);
    assert_eq!(eye_children(first.as_ref(), "right-eye").len(), 1);
    assert_ne!(first, second, "local pose updates must move the eye roots");

    let held = viture
        .frame(&PoseView::identity(3).with_timing(13, 80_000_000))
        .unwrap();
    assert_eq!(held, second, "pose beyond the clamp holds the last frame");

    let mirror_caps = SurfaceCaps::from_preset("glasses-stereo", "viture.mirror").unwrap();
    let mut mirror = VitureSceneClient::new(&mirror_caps, 12).unwrap();
    let mirror_scene = spatial_scene();
    mirror.receive(mirror_scene.clone()).unwrap();
    assert!(mirror.is_mirror());
    assert_eq!(
        mirror.frame(&PoseView::identity(4)).unwrap().as_ref(),
        &mirror_scene
    );

    let halo_caps = SurfaceCaps::from_preset("glasses-hud", "halo.preview").unwrap();
    let mut halo = HaloPreviewClient::new(&halo_caps).unwrap();
    halo.receive(glance_scene()).unwrap();
    let card = halo.frame(&GlanceState::idle(1)).unwrap();
    assert_eq!(halo.content_receipts(), 1);
    assert_scene_kind(card.as_ref(), "glance");
    assert_eq!(
        GlanceCard::from_scene(card.as_ref()).unwrap().title,
        "Build"
    );
}

#[test]
fn glasses_clients_fail_closed_on_wrong_scene_kinds() {
    let viture_caps = SurfaceCaps::from_preset("glasses-luma-ultra", "viture.browser").unwrap();
    let mut viture = VitureSceneClient::new(&viture_caps, 12).unwrap();
    assert!(viture.receive(glance_scene()).is_err());
    assert!(viture.frame(&PoseView::identity(1)).is_err());

    let halo_caps = SurfaceCaps::from_preset("glasses-hud", "halo.preview").unwrap();
    let mut halo = HaloPreviewClient::new(&halo_caps).unwrap();
    assert!(halo.receive(spatial_scene()).is_err());
    assert!(halo.frame(&GlanceState::idle(1)).is_err());
}

fn spatial_scene() -> Expr {
    sim_lib_scene::spatial(vec![sim_lib_scene::panel(
        "workspace",
        sim_lib_scene::node("text", vec![("text", build::text("Workspace"))]),
        Anchor::new(AnchorSpace::World, "desk"),
        Transform3::new([0.0, 0.0, -1.5], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]),
    )])
}

fn glance_scene() -> Expr {
    GlanceCard::new(
        "Build",
        Some(GlanceMetric::new("tests", "green")),
        None,
        "info",
        2,
    )
    .to_scene()
}

fn assert_scene_kind(expr: &Expr, expected: &str) {
    let kind = sim_lib_scene::node_kind(expr).expect("scene kind");
    assert_eq!(kind.namespace.as_deref(), Some("scene"));
    assert_eq!(kind.name.as_ref(), expected);
}

fn eye_children<'a>(scene: &'a Expr, field: &str) -> &'a [Expr] {
    match access::field(access::field(scene, field).unwrap(), "children").unwrap() {
        Expr::List(children) => children,
        other => panic!("expected children list, got {other:?}"),
    }
}

fn field_pair(scene: &Expr, field: &str) -> Option<[u64; 2]> {
    let Expr::List(values) = access::field(scene, field)? else {
        return None;
    };
    Some([number(&values[0])?, number(&values[1])?])
}

fn field_u64(scene: &Expr, field: &str) -> Option<u64> {
    number(access::field(scene, field)?)
}

fn number(expr: &Expr) -> Option<u64> {
    let Expr::Number(number) = expr else {
        return None;
    };
    number.canonical.parse().ok()
}
