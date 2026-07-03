//! Tests for experience modes and capability-aware action exposure.

use sim_kernel::{CapabilityName, Expr, NumberLiteral, Symbol};

use crate::mode::{Exposure, Mode, action_exposure, denied_scene, readonly_scene, universal_scene};

fn doc() -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("a")),
            Expr::Number(NumberLiteral {
                domain: Symbol::new("i64"),
                canonical: "1".to_owned(),
            }),
        ),
        (Expr::Symbol(Symbol::new("b")), Expr::String("x".to_owned())),
    ])
}

fn region_count(scene: &Expr) -> usize {
    let Expr::Map(entries) = scene else { return 0 };
    entries
        .iter()
        .find_map(|(key, value)| {
            let is_children = matches!(key, Expr::Symbol(s) if &*s.name == "children");
            match value {
                Expr::List(items) if is_children => Some(items.len()),
                _ => None,
            }
        })
        .unwrap_or(0)
}

#[test]
fn the_same_value_renders_at_different_depth_per_mode() {
    let value = doc();
    let household = universal_scene(&value, Mode::Household);
    let builder = universal_scene(&value, Mode::Builder);
    let systems = universal_scene(&value, Mode::Systems);

    assert_eq!(region_count(&household), 2);
    assert_eq!(region_count(&builder), 3);
    assert_eq!(region_count(&systems), 4);

    // Every depth is a valid scene, and the value is never changed.
    for scene in [&household, &builder, &systems] {
        sim_lib_scene::validate_scene(scene).expect("mode scene is valid");
    }
    assert_eq!(value, doc(), "rendering never mutates the value");
}

#[test]
fn capability_denied_actions_are_absent_not_disabled() {
    let admin = CapabilityName::new("admin");
    let required = vec![admin.clone()];
    let deny = |c: &CapabilityName| c.as_str() != "admin";
    assert_eq!(
        action_exposure(&required, deny, false, Mode::Systems),
        Exposure::Absent
    );
}

#[test]
fn dangerous_actions_are_confirmation_gated_and_hidden_in_household() {
    let grant = |_: &CapabilityName| true;
    assert_eq!(
        action_exposure(&[], grant, true, Mode::Systems),
        Exposure::ConfirmationGated
    );
    assert_eq!(
        action_exposure(&[], grant, true, Mode::Household),
        Exposure::Absent
    );
    assert_eq!(
        action_exposure(&[], grant, false, Mode::Builder),
        Exposure::Shown
    );
}

#[test]
fn denied_and_readonly_scenes_are_legible_not_blank() {
    let denied = denied_scene("you do not have permission to delete");
    sim_lib_scene::validate_scene(&denied).expect("denied scene is valid");

    let readonly = readonly_scene(&doc(), Mode::Builder);
    sim_lib_scene::validate_scene(&readonly).expect("readonly scene is valid");
}
