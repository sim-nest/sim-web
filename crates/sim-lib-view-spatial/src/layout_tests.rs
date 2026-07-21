use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_lib_scene::{AnchorSpace, Transform3};
use sim_lib_view::{SurfaceCaps, SurfaceCodec};
use sim_table_core::{TableOp, TablePath, decode_table_op, encode_table_op};
use sim_value::{access, build};

use crate::{
    GlancePreference, PanelPlacement, SpatialSurfaceCodec, WorkspaceLayout, layout_load_op,
    layout_save_op, layout_table_key,
};

#[test]
fn workspace_layout_roundtrips_through_table_op() {
    let layout = workspace_layout();
    let path = layout_path();
    let save = layout_save_op(&path, &layout);
    let encoded = encode_table_op(&save);
    let decoded = decode_table_op(&encoded).expect("table/set decodes");

    let TableOp::Set(key, expr) = decoded else {
        panic!("expected table/set");
    };
    assert_eq!(key, layout_table_key(&path));
    let parsed = WorkspaceLayout::from_expr(&expr).expect("layout parses");
    assert_eq!(parsed, layout);

    let TableOp::Get(load_key) = layout_load_op(&path) else {
        panic!("expected table/get");
    };
    assert_eq!(load_key, key);
}

#[test]
fn encoder_reads_workspace_layout_and_uses_default_arc_when_missing() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    let codec = SpatialSurfaceCodec::new();
    let caps = SurfaceCaps::from_preset("glasses-luma-ultra", "viture").unwrap();
    let layout = workspace_layout();

    let value = build::map(vec![
        ("title", Expr::String("Spatial plan".to_owned())),
        ("workspace-layout", layout.to_expr()),
    ]);
    let scene = codec.encode(&mut cx, &value, &caps).expect("encodes");
    let panel = first_panel(&scene);
    assert_eq!(access::field_str(panel, "id"), Some("main"));
    let restored =
        Transform3::from_expr(access::required(panel, "transform", "scene/panel").unwrap())
            .expect("panel transform parses");
    assert_eq!(restored, layout.panels()[0].transform);
    assert_eq!(
        layout.glance().preferred_item_class(),
        Some(&Symbol::qualified("intent", "invoke"))
    );

    let fallback = codec
        .encode(
            &mut cx,
            &build::map(vec![("title", Expr::String("Default".to_owned()))]),
            &caps,
        )
        .expect("encodes with default layout");
    let default_panel = first_panel(&fallback);
    let default_transform =
        Transform3::from_expr(access::required(default_panel, "transform", "scene/panel").unwrap())
            .expect("default transform parses");
    assert_eq!(
        default_transform,
        WorkspaceLayout::default_arc().panels()[0].transform
    );
}

fn workspace_layout() -> WorkspaceLayout {
    WorkspaceLayout::new(
        vec![
            PanelPlacement::new(
                Symbol::new("main"),
                AnchorSpace::World,
                Transform3::new([0.0, 1.1, -1.7], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]),
            )
            .with_world_anchor(Symbol::new("desk")),
            PanelPlacement::new(
                Symbol::new("notes"),
                AnchorSpace::World,
                Transform3::new([0.6, 1.0, -1.5], [0.0, 0.0, 0.0, 1.0], [0.8, 0.8, 0.8]),
            )
            .with_world_anchor(Symbol::new("desk")),
        ],
        GlancePreference::item_class(Symbol::qualified("intent", "invoke")),
    )
    .expect("layout is valid")
}

fn layout_path() -> TablePath {
    let mut path = TablePath::new();
    path.push("sessions").unwrap();
    path.push("co-use").unwrap();
    path
}

fn first_panel(scene: &Expr) -> &Expr {
    let Expr::List(children) = access::required(scene, "children", "scene/spatial").unwrap() else {
        panic!("scene children must be a list");
    };
    children.first().expect("at least one panel")
}
