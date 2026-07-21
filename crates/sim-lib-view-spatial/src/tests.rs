use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr};
use sim_lib_scene::{Anchor, AnchorSpace, GLANCE_KIND, Transform3};
use sim_lib_view::{SurfaceCaps, SurfaceCodec, UniversalView, View};
use sim_lib_view_device::{DeviceSurfaceCapsExt, GlanceReducer};
use sim_value::{access, build};

use crate::SpatialSurfaceCodec;

#[test]
fn encoders_are_pose_free_and_halo_uses_glance_reducer() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    let codec = SpatialSurfaceCodec::new();
    let value = build::map(vec![
        ("title", Expr::String("Daily focus".to_owned())),
        ("spatial-layout", layout()),
    ]);

    let viture = SurfaceCaps::from_preset("glasses-luma-ultra", "viture").unwrap();
    let viture_scene = codec.encode(&mut cx, &value, &viture).unwrap();
    assert_scene_kind(&viture_scene, "spatial");
    assert_pose_free(&viture_scene);
    let first_panel = first_child(&viture_scene);
    assert_scene_kind(first_panel, "panel");
    assert_eq!(access::field_str(first_panel, "id"), Some("main"));

    let halo = SurfaceCaps::from_preset("glasses-hud", "halo").unwrap();
    let halo_scene = codec.encode(&mut cx, &value, &halo).unwrap();
    assert_scene_kind(&halo_scene, GLANCE_KIND);
    assert_pose_free(&halo_scene);
    let source_scene = UniversalView
        .encode(
            &mut cx,
            &build::map(vec![("title", Expr::String("Daily focus".to_owned()))]),
        )
        .unwrap();
    let expected_halo = GlanceReducer
        .reduce(&source_scene, &halo.device_profile())
        .unwrap();
    assert_eq!(halo_scene, expected_halo);

    let display = SurfaceCaps::from_preset("glasses", "mirror").unwrap();
    let display_scene = codec.encode(&mut cx, &value, &display).unwrap();
    assert_scene_kind(&display_scene, "stack");
    assert_pose_free(&display_scene);
}

fn layout() -> Expr {
    build::map(vec![(
        "panels",
        build::list(vec![build::map(vec![
            ("id", Expr::String("main".to_owned())),
            ("anchor", Anchor::new(AnchorSpace::World, "desk").to_expr()),
            (
                "transform",
                Transform3::new([0.0, 1.2, -1.6], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]).to_expr(),
            ),
        ])]),
    )])
}

fn assert_scene_kind(scene: &Expr, expected: &str) {
    let kind = sim_lib_scene::node_kind(scene).expect("scene kind");
    assert_eq!(kind.namespace.as_deref(), Some("scene"));
    assert_eq!(kind.name.as_ref(), expected);
}

fn first_child(scene: &Expr) -> &Expr {
    match access::field(scene, "children").expect("children") {
        Expr::List(children) => children.first().expect("at least one child"),
        other => panic!("children must be a list, got {other:?}"),
    }
}

fn assert_pose_free(expr: &Expr) {
    match expr {
        Expr::Map(entries) => {
            for (key, value) in entries {
                if let Expr::Symbol(symbol) = key {
                    assert_ne!(symbol.name.as_ref(), "pose");
                    assert_ne!(symbol.name.as_ref(), "tick");
                }
                assert_pose_free(value);
            }
        }
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => {
            for item in items {
                assert_pose_free(item);
            }
        }
        _ => {}
    }
}
