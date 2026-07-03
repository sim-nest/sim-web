//! Tests for arranger object-roll Scene descriptors.

use sim_kernel::Expr;

use crate::{ARRANGER_OBJECT_ROLL_ACTIONS, arranger_object_roll_demo_scene};

#[test]
fn arranger_object_roll_demo_covers_placement_handles() {
    let scene = arranger_object_roll_demo_scene();
    sim_lib_scene::validate_scene(&scene).expect("arranger object-roll scene is valid");
    assert!(sim_test_support::contains_kind(&scene, "object-roll"));
    assert_eq!(
        field(&scene, "role"),
        Some(Expr::Symbol(sim_kernel::Symbol::new(
            "arranger-object-roll"
        )))
    );
    for action in ARRANGER_OBJECT_ROLL_ACTIONS {
        assert!(string_list_contains(&scene, "actions", action));
    }

    let placements = list_field(&list_field(&scene, "lanes")[0], "placements");
    let motif = placements.first().expect("motif placement");
    assert!(field(motif, "at").is_some());
    assert!(field(motif, "duration").is_some());
    assert_eq!(
        field(motif, "stretch"),
        Some(Expr::String("fit-to-duration".to_owned()))
    );
    assert!(field(motif, "transpose").is_some());
    assert_eq!(
        field(motif, "invert"),
        Some(Expr::String("pitch:C4".to_owned()))
    );
    assert_eq!(field(motif, "retrograde"), Some(Expr::Bool(true)));
    assert_eq!(
        field(motif, "remap-pitch"),
        Some(Expr::String("scale:minor-pentatonic".to_owned()))
    );
    assert!(field(motif, "filter").is_some());
    assert!(field(motif, "target").is_some());
    assert!(field(motif, "seed").is_some());
    assert_eq!(
        field(motif, "trace-policy"),
        Some(Expr::String("full".to_owned()))
    );
    assert!(string_list_contains(motif, "freeze-targets", "piano-roll"));
    assert!(string_list_contains(motif, "freeze-targets", "midi"));
}

#[test]
fn arranger_object_roll_demo_covers_nested_diagnostics_and_remaps() {
    let scene = arranger_object_roll_demo_scene();
    let lanes = list_field(&scene, "lanes");
    let all_placements = lanes
        .iter()
        .flat_map(|lane| list_field(lane, "placements"))
        .collect::<Vec<_>>();
    assert!(
        all_placements
            .iter()
            .any(|placement| field(placement, "nested") == Some(Expr::Bool(true)))
    );
    for remap in [
        "scale:minor-pentatonic",
        "vector:modal-axis",
        "matrix:ps3300-map",
    ] {
        assert!(
            all_placements
                .iter()
                .any(|placement| field(placement, "remap-pitch")
                    == Some(Expr::String(remap.to_owned()))),
            "missing remap {remap}"
        );
    }

    let diagnostics = list_field(&scene, "diagnostics");
    for kind in [
        "dropped-event",
        "missing-capability",
        "impossible-remap",
        "clipped-range",
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| field(diagnostic, "diagnostic-kind")
                    == Some(Expr::String(kind.to_owned()))),
            "missing diagnostic {kind}"
        );
    }
}

fn field(map: &Expr, name: &str) -> Option<Expr> {
    let Expr::Map(entries) = map else {
        return None;
    };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(s) if &*s.name == name).then(|| value.clone())
    })
}

fn list_field(map: &Expr, name: &str) -> Vec<Expr> {
    match field(map, name) {
        Some(Expr::List(items)) => items,
        _ => Vec::new(),
    }
}

fn string_list_contains(map: &Expr, name: &str, expected: &str) -> bool {
    list_field(map, name)
        .iter()
        .any(|item| matches!(item, Expr::String(text) if text == expected))
}
