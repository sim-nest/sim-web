//! Tests for lens contracts, Shape-based dispatch, and render/propose.

use std::sync::Arc;

use sim_kernel::{CapabilityName, Cx, Expr, Result, Symbol};
use sim_lib_intent::Origin;
use sim_lib_scene::{scene_shape_specs, scene_shape_symbol};
use sim_shape::{AnyShape, shape_value};

use crate::contract::{Draft, Editor, Lens, LensKind, LensMeta, Operation, View};
use crate::dispatch::{DispatchContext, DispatchReason, LensRegistry};

use sim_kernel::testing::eager_cx as cx;

use sim_value::build::keyword as sym;

fn node_shape(name: &str) -> Expr {
    // returns a scene of kind scene/<name> to dispatch against
    sim_lib_scene::node(name, vec![("id", Expr::Symbol(sym("x")))])
}

fn scene_node_shape_value(name: &str) -> sim_kernel::Value {
    let symbol = Symbol::qualified("scene", capitalize(name));
    shape_value(symbol.clone(), shape_for_symbol(symbol))
}

fn umbrella_scene_shape_value() -> sim_kernel::Value {
    let symbol = scene_shape_symbol();
    shape_value(symbol.clone(), shape_for_symbol(symbol))
}

fn shape_for_symbol(symbol: Symbol) -> Arc<dyn sim_kernel::Shape> {
    scene_shape_specs()
        .into_iter()
        .find(|(candidate, _)| candidate == &symbol)
        .map(|(_, shape)| shape)
        .unwrap_or_else(|| panic!("missing scene shape {symbol}"))
}

fn any_shape_value() -> sim_kernel::Value {
    shape_value(Symbol::qualified("core", "Any"), Arc::new(AnyShape))
}

fn capitalize(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn grant_all(_: &CapabilityName) -> bool {
    true
}

/// A registry with a universal default, a generic scene view, and a specific
/// graph view.
fn registry() -> LensRegistry {
    let mut registry = LensRegistry::new();
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:default"), LensKind::View)
            .claiming_shape(any_shape_value())
            .with_quality_cost(-100, 0)
            .as_universal_default(),
    ));
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:scene-generic"), LensKind::View)
            .claiming_shape(umbrella_scene_shape_value())
            .with_quality_cost(0, 10),
    ));
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:graph"), LensKind::View)
            .claiming_shape(scene_node_shape_value("graph"))
            .with_quality_cost(10, 5),
    ));
    registry
}

fn ctx<'a>() -> DispatchContext<'a> {
    DispatchContext::permissive(&grant_all)
}

#[test]
fn most_specific_shape_wins() {
    let mut cx = cx();
    let registry = registry();
    let outcome = registry
        .dispatch_view(&mut cx, &node_shape("graph"), &ctx())
        .unwrap();
    assert_eq!(outcome.lens_id, sym("view:graph"));
    assert_eq!(outcome.reason, DispatchReason::ShapeMatch(20));
}

#[test]
fn falls_back_to_generic_then_universal() {
    let mut cx = cx();
    let registry = registry();
    // A box scene: only the umbrella scene shape matches.
    let outcome = registry
        .dispatch_view(&mut cx, &node_shape("box"), &ctx())
        .unwrap();
    assert_eq!(outcome.lens_id, sym("view:scene-generic"));
    assert_eq!(outcome.reason, DispatchReason::ShapeMatch(5));

    // A non-scene value: nothing matches, universal default catches it.
    let outcome = registry
        .dispatch_view(&mut cx, &Expr::String("plain".to_owned()), &ctx())
        .unwrap();
    assert_eq!(outcome.lens_id, sym("view:default"));
    assert_eq!(outcome.reason, DispatchReason::UniversalDefault);
}

#[test]
fn explicit_choice_and_preference_take_priority() {
    let mut cx = cx();
    let registry = registry();
    let mut context = ctx();
    context.explicit = Some(sym("view:scene-generic"));
    let outcome = registry
        .dispatch_view(&mut cx, &node_shape("graph"), &context)
        .unwrap();
    assert_eq!(outcome.lens_id, sym("view:scene-generic"));
    assert_eq!(outcome.reason, DispatchReason::Explicit);

    let mut context = ctx();
    context.preference = Some(sym("view:default"));
    let outcome = registry
        .dispatch_view(&mut cx, &node_shape("graph"), &context)
        .unwrap();
    assert_eq!(outcome.lens_id, sym("view:default"));
    assert_eq!(outcome.reason, DispatchReason::Preference);
}

#[test]
fn ties_break_by_quality_then_cost() {
    let mut cx = cx();
    let mut registry = LensRegistry::new();
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:graph-low"), LensKind::View)
            .claiming_shape(scene_node_shape_value("graph"))
            .with_quality_cost(1, 1),
    ));
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:graph-high"), LensKind::View)
            .claiming_shape(scene_node_shape_value("graph"))
            .with_quality_cost(5, 1),
    ));
    let outcome = registry
        .dispatch_view(&mut cx, &node_shape("graph"), &ctx())
        .unwrap();
    assert_eq!(outcome.lens_id, sym("view:graph-high"));
}

#[test]
fn a_denied_lens_falls_through_to_the_next_candidate() {
    let mut cx = cx();
    let mut registry = LensRegistry::new();
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:default"), LensKind::View)
            .claiming_shape(any_shape_value())
            .with_quality_cost(-100, 0)
            .as_universal_default(),
    ));
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:graph-admin"), LensKind::View)
            .claiming_shape(scene_node_shape_value("graph"))
            .with_quality_cost(50, 1)
            .requiring(CapabilityName::new("admin")),
    ));
    // Without the capability, the specific lens is skipped -> universal default.
    let deny = |capability: &CapabilityName| capability.as_str() != "admin";
    let mut context = ctx();
    context.granted = &deny;
    let outcome = registry
        .dispatch_view(&mut cx, &node_shape("graph"), &context)
        .unwrap();
    assert_eq!(outcome.lens_id, sym("view:default"));

    // With the capability, the specific lens wins.
    let outcome = registry
        .dispatch_view(&mut cx, &node_shape("graph"), &ctx())
        .unwrap();
    assert_eq!(outcome.lens_id, sym("view:graph-admin"));
}

#[test]
fn class_match_sits_between_shape_match_and_universal_default() {
    let mut cx = cx();
    let mut registry = LensRegistry::new();
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:default"), LensKind::View)
            .claiming_shape(any_shape_value())
            .with_quality_cost(-100, 0)
            .as_universal_default(),
    ));
    registry.register(Lens::metadata_only(
        LensMeta::new(sym("view:foo"), LensKind::View)
            .claiming_class(Symbol::qualified("core", "Foo"))
            .with_quality_cost(3, 1),
    ));
    let mut context = ctx();
    context.value_class = Some(Symbol::qualified("core", "Foo"));
    // A plain value with no matching shape but the right class.
    let outcome = registry
        .dispatch_view(&mut cx, &Expr::String("plain".to_owned()), &context)
        .unwrap();
    assert_eq!(outcome.lens_id, sym("view:foo"));
    assert_eq!(outcome.reason, DispatchReason::ClassMatch);
}

#[test]
fn no_candidate_at_all_is_an_error() {
    let mut cx = cx();
    let registry = LensRegistry::new();
    let result = registry.dispatch_view(&mut cx, &Expr::Nil, &ctx());
    assert!(result.is_err(), "an empty registry cannot dispatch");
}

struct BoxView;

impl View for BoxView {
    fn encode(&self, _cx: &mut Cx, _value: &Expr) -> Result<Expr> {
        Ok(sim_lib_scene::node(
            "box",
            vec![("label", Expr::String("hello".to_owned()))],
        ))
    }
}

struct BadView;

impl View for BadView {
    fn encode(&self, _cx: &mut Cx, _value: &Expr) -> Result<Expr> {
        // A map with no kind tag is not a valid scene.
        Ok(sim_lib_scene::map(vec![("not", Expr::Nil)]))
    }
}

struct PassthroughEditor;

impl Editor for PassthroughEditor {
    fn decode(&self, _cx: &mut Cx, value: &Expr, _intent: &Expr) -> Result<Draft> {
        Ok(Draft::clean(value.clone(), value.clone()))
    }

    fn commit(&self, _cx: &mut Cx, draft: &Draft) -> Result<Operation> {
        Ok(Operation {
            form: draft.proposed.clone(),
        })
    }
}

#[test]
fn render_validates_the_emitted_scene() {
    let mut cx = cx();
    let mut registry = LensRegistry::new();
    registry.register(Lens::view(
        LensMeta::new(sym("view:box"), LensKind::View),
        Arc::new(BoxView),
    ));
    registry.register(Lens::view(
        LensMeta::new(sym("view:bad"), LensKind::View),
        Arc::new(BadView),
    ));
    let scene = registry
        .render(&mut cx, &sym("view:box"), &Expr::Nil)
        .unwrap();
    assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    assert!(
        registry
            .render(&mut cx, &sym("view:bad"), &Expr::Nil)
            .is_err(),
        "an invalid scene must fail closed"
    );
}

#[test]
fn propose_validates_the_intent_before_the_editor_sees_it() {
    let mut cx = cx();
    let mut registry = LensRegistry::new();
    registry.register(Lens::editor(
        LensMeta::new(sym("edit:passthrough"), LensKind::Editor),
        Arc::new(PassthroughEditor),
    ));
    // A malformed intent is rejected before reaching the editor.
    let bad_intent = Expr::Map(vec![]);
    assert!(
        registry
            .propose(&mut cx, &sym("edit:passthrough"), &Expr::Nil, &bad_intent)
            .is_err()
    );
    // A valid intent yields a draft that commits.
    let intent = sim_lib_intent::intent(
        "select",
        Origin::human(1),
        vec![("targets", Expr::List(vec![]))],
    );
    let draft = registry
        .propose(&mut cx, &sym("edit:passthrough"), &Expr::Nil, &intent)
        .unwrap();
    assert!(draft.committable);
    let op = registry
        .commit(&mut cx, &sym("edit:passthrough"), &draft)
        .unwrap();
    assert_eq!(op.form, Expr::Nil);
}
