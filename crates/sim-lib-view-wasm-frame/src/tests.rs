//! Tests for the local view host facade.

use sim_kernel::{Expr, NumberLiteral, Symbol};
use sim_lib_intent::gesture::{Hit, HitRole, PointerEvent, PointerPhase};
use sim_lib_intent::intent_kind_of;

use crate::host::BrowserHost;

use sim_value::build::sym;

fn number(value: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::new("i64"),
        canonical: value.to_owned(),
    })
}

fn doc() -> Expr {
    Expr::Map(vec![(sym("a"), number("1")), (sym("b"), number("2"))])
}

#[test]
fn a_value_renders_the_universal_default_lens() {
    let host = BrowserHost::new(doc()).unwrap();
    sim_lib_scene::validate_scene(host.scene()).expect("the host renders a valid scene");
}

#[test]
fn a_field_edit_gesture_commits_and_updates_the_view() {
    let mut host = BrowserHost::new(doc()).unwrap();
    let initial = host.scene().clone();
    let update = host
        .edit_field(
            Expr::List(vec![Expr::Vector(vec![sym("k"), sym("a")])]),
            number("9"),
        )
        .unwrap()
        .expect("a field edit mutates and updates the view");
    // The value changed.
    let Expr::Map(entries) = host.value() else {
        panic!("doc is a map")
    };
    let a = entries
        .iter()
        .find(|(k, _)| matches!(k, Expr::Symbol(s) if &*s.name == "a"))
        .map(|(_, v)| v);
    assert_eq!(a, Some(&number("9")));
    // The diff reconstructs the new scene from the old one.
    let rebuilt = sim_lib_scene::apply(&initial, &update.diff).unwrap();
    assert_eq!(rebuilt, update.scene);
}

#[test]
fn a_pointer_tap_composes_a_valid_select_intent() {
    let mut host = BrowserHost::new(doc()).unwrap();
    let hit = Hit::on(HitRole::Node, sym("a"));
    assert!(
        host.feed_pointer(down(5.0, 5.0, hit.clone()))
            .unwrap()
            .is_none()
    );
    let intent = host
        .feed_pointer(up(6.0, 6.0, hit))
        .unwrap()
        .expect("the release completes a gesture");
    assert_eq!(
        intent_kind_of(&intent).map(|s| s.name.to_string()),
        Some("select".to_owned())
    );
    sim_lib_intent::validate_intent(&intent).expect("composed intent is valid");
    // Selecting does not mutate the universal value.
    assert!(host.apply_intent(&intent).unwrap().is_none());
}

fn event(phase: PointerPhase, x: f64, y: f64, hit: Hit) -> PointerEvent {
    PointerEvent { phase, x, y, hit }
}

fn down(x: f64, y: f64, hit: Hit) -> PointerEvent {
    event(PointerPhase::Down, x, y, hit)
}

fn up(x: f64, y: f64, hit: Hit) -> PointerEvent {
    event(PointerPhase::Up, x, y, hit)
}
