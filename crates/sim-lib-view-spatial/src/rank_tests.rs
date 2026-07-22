use sim_kernel::Expr;
use sim_lib_scene::{Anchor, AnchorSpace, GLANCE_KIND, GlanceCard, Transform3};
use sim_lib_view::SurfaceCaps;
use sim_lib_view_device::{DeviceSurfaceCapsExt, GlassesClass};
use sim_value::access;

use crate::{AttentionBudget, halo_glance_scene, rank_glasses, rank_spatial};

#[test]
fn spatial_collapses_periphery_glance_stays_one_item() {
    let budget = AttentionBudget {
        foveal_deg: 10.0,
        peripheral_deg: 60.0,
        max_lit_panels: 2,
        camera_live: true,
    };
    let ranked = rank_spatial(&spatial_scene(), [0.0, 0.0, -1.0], &budget).unwrap();
    assert_scene_kind(&ranked, "spatial");
    assert_eq!(access::field_bool(&ranked, "privacy-shade"), Some(true));
    assert_symbol_token(&ranked, "privacy-reason", "camera-live");

    let focus = panel(&ranked, "focus");
    assert_symbol_token(focus, "attention-detail", "foveal");
    assert_eq!(access::field_bool(focus, "attention-lit"), Some(true));

    let near = panel(&ranked, "near");
    assert_symbol_token(near, "attention-detail", "peripheral");
    assert_eq!(access::field_bool(near, "attention-lit"), Some(true));

    let over_budget = panel(&ranked, "over-budget");
    assert_symbol_token(over_budget, "attention-detail", "hidden");
    assert_symbol_token(over_budget, "attention-reason", "budget");
    assert_eq!(
        access::field_bool(over_budget, "attention-lit"),
        Some(false)
    );

    let warrant = panel(&ranked, "warrant");
    assert_eq!(access::field_bool(warrant, "attention-lit"), Some(true));
    assert_eq!(access::field_bool(warrant, "attention-pinned"), Some(true));
    assert_symbol_token(warrant, "attention-reason", "pinned");
    assert_eq!(normal_lit_count(&ranked), 2);

    let halo_profile = SurfaceCaps::from_preset("glasses-hud", "halo")
        .unwrap()
        .device_profile();
    let glance = halo_glance_scene(&source_scene_with_warrant(), &halo_profile).unwrap();
    assert_scene_kind(&glance, GLANCE_KIND);
    assert!(access::field(&glance, "children").is_none());
    assert_eq!(access::field_bool(&glance, "bypass-budget"), Some(true));
    assert_symbol_token(&glance, "urgency", "error");

    let halo_ranked =
        rank_glasses(&glance, GlassesClass::MonoHud, [0.0, 0.0, -1.0], &budget).unwrap();
    assert_eq!(halo_ranked, glance);
}

fn spatial_scene() -> Expr {
    sim_lib_scene::spatial(vec![
        panel_node("focus", sim_lib_scene::text_node("Focus"), [0.0, 0.0, -1.0]),
        panel_node("near", sim_lib_scene::text_node("Near"), [0.3, 0.0, -1.0]),
        panel_node(
            "over-budget",
            sim_lib_scene::text_node("Overflow"),
            [0.6, 0.0, -1.0],
        ),
        panel_node(
            "warrant",
            sim_lib_scene::badge("error", "Doorbell camera"),
            [6.0, 0.0, 0.0],
        ),
    ])
}

fn source_scene_with_warrant() -> Expr {
    sim_lib_scene::stack(
        "column",
        vec![
            sim_lib_scene::text_node("Door"),
            sim_lib_scene::badge("error", "Doorbell camera"),
            sim_lib_scene::text_node("Later item"),
        ],
    )
}

fn panel_node(id: &str, body: Expr, translate_m: [f64; 3]) -> Expr {
    sim_lib_scene::panel(
        id,
        body,
        Anchor::new(AnchorSpace::World, id),
        Transform3::new(translate_m, [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]),
    )
}

fn panel<'a>(scene: &'a Expr, id: &str) -> &'a Expr {
    spatial_children(scene)
        .iter()
        .find(|panel| access::field_str(panel, "id") == Some(id))
        .unwrap_or_else(|| panic!("missing panel {id}"))
}

fn spatial_children(scene: &Expr) -> &[Expr] {
    match access::field(scene, "children").expect("children") {
        Expr::List(children) => children,
        other => panic!("children must be a list, got {other:?}"),
    }
}

fn normal_lit_count(scene: &Expr) -> usize {
    spatial_children(scene)
        .iter()
        .filter(|panel| access::field_bool(panel, "attention-lit") == Some(true))
        .filter(|panel| access::field_bool(panel, "attention-pinned") != Some(true))
        .count()
}

fn assert_scene_kind(scene: &Expr, expected: &str) {
    let kind = sim_lib_scene::node_kind(scene).expect("scene kind");
    assert_eq!(kind.namespace.as_deref(), Some("scene"));
    assert_eq!(kind.name.as_ref(), expected);
}

fn assert_symbol_token(expr: &Expr, name: &str, expected: &str) {
    let symbol = access::field_sym(expr, name).unwrap_or_else(|| panic!("missing symbol {name}"));
    assert_eq!(symbol.name.as_ref(), expected);
}

#[test]
fn rank_glasses_leaves_display_only_paths_unchanged() {
    let scene = GlanceCard::new("Mirror", None, None, "info", 1).to_scene();
    let ranked = rank_glasses(
        &scene,
        GlassesClass::DisplayOnly,
        [0.0, 0.0, -1.0],
        &AttentionBudget::new(1),
    )
    .unwrap();
    assert_eq!(ranked, scene);
}
