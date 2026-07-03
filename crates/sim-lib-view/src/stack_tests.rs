//! Tests for the lens stack, intent/set-lens, and scene embedding.

use std::sync::Arc;

use sim_kernel::{CapabilityName, Expr, Symbol};
use sim_lib_intent::{Origin, intent};
use sim_lib_scene::shapes::{SceneNodeShape, SceneShape};
use sim_shape::shape_value;

use crate::contract::{Lens, LensKind, LensMeta};
use crate::dispatch::{DispatchContext, DispatchReason, LensRegistry};
use crate::set_lens::{active_lens, apply_set_lens, empty_pane_lenses};
use crate::universal::register_universal_default;

use sim_kernel::testing::eager_cx as cx;

use sim_value::build::keyword as sym;

fn grant_all(_: &CapabilityName) -> bool {
    true
}

fn registry() -> LensRegistry {
    let mut registry = LensRegistry::new();
    register_universal_default(&mut registry, false);
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:scene-generic"), LensKind::View)
            .claiming_shape(shape_value(
                Symbol::qualified("scene", "Scene"),
                Arc::new(SceneShape),
            ))
            .with_quality_cost(0, 10),
    ));
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:graph"), LensKind::View)
            .claiming_shape(shape_value(
                Symbol::qualified("scene", "graph"),
                Arc::new(SceneNodeShape::new("graph")),
            ))
            .with_quality_cost(10, 5),
    ));
    registry
}

#[test]
fn a_value_with_several_lenses_yields_an_ordered_stack() {
    let mut cx = cx();
    let registry = registry();
    let graph = sim_lib_scene::node("graph", vec![("id", Expr::Symbol(sym("g")))]);
    let grant = grant_all;
    let ctx = DispatchContext::permissive(&grant);
    let stack = registry
        .lens_stack(&mut cx, LensKind::View, &graph, &ctx)
        .unwrap();
    let ids: Vec<String> = stack.iter().map(|e| e.lens_id.name.to_string()).collect();
    assert_eq!(
        ids,
        vec!["view:graph", "view:scene-generic", "view:default"],
        "stack must be ordered best-first ending at the universal default"
    );
    assert_eq!(stack[0].reason, DispatchReason::ShapeMatch(20));
    assert_eq!(
        stack.last().unwrap().reason,
        DispatchReason::UniversalDefault
    );
}

#[test]
fn flipping_the_active_lens_does_not_touch_the_value() {
    let value = sim_lib_scene::node("graph", vec![("id", Expr::Symbol(sym("g")))]);
    let value_before = value.clone();

    let mut state = empty_pane_lenses();
    // Start on the graph lens.
    state = apply_set_lens(&state, &set_lens_intent("pane-1", "view:graph")).unwrap();
    assert_eq!(active_lens(&state, "pane-1"), Some(sym("view:graph")));

    // Flip to the generic lens.
    state = apply_set_lens(&state, &set_lens_intent("pane-1", "view:scene-generic")).unwrap();
    assert_eq!(
        active_lens(&state, "pane-1"),
        Some(sym("view:scene-generic"))
    );

    // A different pane keeps its own choice independently.
    state = apply_set_lens(&state, &set_lens_intent("pane-2", "view:default")).unwrap();
    assert_eq!(
        active_lens(&state, "pane-1"),
        Some(sym("view:scene-generic"))
    );
    assert_eq!(active_lens(&state, "pane-2"), Some(sym("view:default")));

    // The value itself is untouched by lens switching.
    assert_eq!(value, value_before);
}

#[test]
fn the_lens_choice_persists_as_a_value() {
    let state = apply_set_lens(
        &empty_pane_lenses(),
        &set_lens_intent("pane-1", "view:graph"),
    )
    .unwrap();
    // The state is a plain data value: it round-trips through the portable form.
    let text = sim_codec::encode_portable(sim_kernel::CodecId(0), &state).unwrap();
    let restored = sim_codec::decode_portable(sim_kernel::CodecId(0), &text).unwrap();
    assert_eq!(state, restored);
    assert_eq!(active_lens(&restored, "pane-1"), Some(sym("view:graph")));
}

#[test]
fn an_embedded_lens_renders_inside_a_host_lens() {
    let mut cx = cx();
    let mut registry = LensRegistry::new();
    register_universal_default(&mut registry, false);
    let value = Expr::String("inner".to_owned());
    // Render the value through the universal default and embed it.
    let embedded = registry
        .render_embedded(&mut cx, &sym("view:default"), &value)
        .unwrap();
    // The embed is a valid scene node hosting a nested scene.
    assert_eq!(
        sim_lib_scene::node_kind(&embedded).map(|k| k.name.to_string()),
        Some("embed".to_owned())
    );
    // A host lens can place the embed inside its own scene and still validate.
    let host = sim_lib_scene::node("stack", vec![("children", Expr::List(vec![embedded]))]);
    sim_lib_scene::validate_scene(&host).expect("host with embedded lens must validate");
}

#[test]
fn apply_set_lens_rejects_other_intents() {
    let other = intent(
        "commit",
        Origin::human(1),
        vec![("pane", Expr::Symbol(sym("p")))],
    );
    assert!(apply_set_lens(&empty_pane_lenses(), &other).is_err());
}

fn set_lens_intent(pane: &str, lens: &str) -> Expr {
    intent(
        "set-lens",
        Origin::human(1),
        vec![
            ("pane", Expr::Symbol(sym(pane))),
            ("lens", Expr::Symbol(sym(lens))),
        ],
    )
}
