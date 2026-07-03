//! Tests for piano-roll and player-rack Scene descriptors.

use sim_kernel::{Expr, Symbol};

use crate::{
    PIANO_ROLL_EDIT_ACTIONS, PLAYER_RACK_ACTIONS, performance_workbench_demo_scene,
    piano_roll_demo_scene, player_rack_demo_scene,
};

#[test]
fn piano_roll_demo_covers_lane_families_and_live_output() {
    let scene = piano_roll_demo_scene();
    sim_lib_scene::validate_scene(&scene).expect("piano-roll demo scene is valid");
    assert!(sim_test_support::contains_kind(&scene, "piano-roll"));

    let lanes = list_field(&scene, "lanes");
    for lane_kind in ["note", "drum", "scale-degree", "object", "automation"] {
        assert!(
            lanes
                .iter()
                .any(|lane| field(lane, "lane-kind") == Some(Expr::String(lane_kind.to_owned()))),
            "lane kind {lane_kind} is present"
        );
    }
    for action in PIANO_ROLL_EDIT_ACTIONS {
        assert!(string_list_contains(&scene, "edit-actions", action));
    }
    assert!(event_flag(&lanes, "live"));
    assert!(event_flag(&lanes, "generated"));
    assert!(!list_field(&scene, "live-notes").is_empty());
    assert!(!list_field(&scene, "generated-notes").is_empty());
}

#[test]
fn player_rack_demo_covers_chain_controls_and_routing() {
    let scene = player_rack_demo_scene();
    sim_lib_scene::validate_scene(&scene).expect("player-rack demo scene is valid");
    assert!(sim_test_support::contains_kind(&scene, "player-rack"));

    for action in PLAYER_RACK_ACTIONS {
        assert!(string_list_contains(&scene, "actions", action));
    }
    assert_eq!(
        field(&scene, "player-chain"),
        Some(Expr::Symbol(Symbol::qualified(
            "music/player-chain",
            "onscreen-keyboard"
        )))
    );

    let players = list_field(&scene, "players");
    assert_eq!(players.len(), 3);
    assert!(
        players
            .iter()
            .all(|player| field(player, "route").is_some())
    );
    assert!(
        players
            .iter()
            .all(|player| field(player, "placement-hint").is_some())
    );
    assert!(
        players
            .iter()
            .any(|player| field(player, "bypassed") == Some(Expr::Bool(true)))
    );
    assert!(
        players
            .iter()
            .any(|player| field(player, "direct-record") == Some(Expr::Bool(true)))
    );
    assert!(
        players
            .iter()
            .any(|player| field(player, "frozen") == Some(Expr::Bool(true)))
    );
    assert!(
        players
            .iter()
            .any(|player| field(player, "trace") == Some(Expr::Bool(true)))
    );
}

#[test]
fn performance_workbench_stacks_keyboard_rack_and_roll() {
    let scene = performance_workbench_demo_scene();
    sim_lib_scene::validate_scene(&scene).expect("workbench demo scene is valid");
    assert!(sim_test_support::contains_kind(&scene, "keyboard"));
    assert!(sim_test_support::contains_kind(&scene, "player-rack"));
    assert!(sim_test_support::contains_kind(&scene, "piano-roll"));
    assert!(contains_symbol(
        &scene,
        "music/player-chain",
        "onscreen-keyboard"
    ));
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

fn event_flag(lanes: &[Expr], flag: &str) -> bool {
    lanes.iter().any(|lane| {
        list_field(lane, "events")
            .iter()
            .any(|event| field(event, flag) == Some(Expr::Bool(true)))
    })
}

fn contains_symbol(expr: &Expr, namespace: &str, name: &str) -> bool {
    match expr {
        Expr::Symbol(symbol)
            if symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name =>
        {
            true
        }
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) | Expr::Block(items) => items
            .iter()
            .any(|item| contains_symbol(item, namespace, name)),
        Expr::Map(entries) => entries.iter().any(|(key, value)| {
            contains_symbol(key, namespace, name) || contains_symbol(value, namespace, name)
        }),
        Expr::Call { operator, args } => {
            contains_symbol(operator, namespace, name)
                || args.iter().any(|arg| contains_symbol(arg, namespace, name))
        }
        Expr::Infix { left, right, .. } => {
            contains_symbol(left, namespace, name) || contains_symbol(right, namespace, name)
        }
        Expr::Prefix { arg, .. } | Expr::Postfix { arg, .. } => {
            contains_symbol(arg, namespace, name)
        }
        Expr::Quote { expr, .. } => contains_symbol(expr, namespace, name),
        Expr::Annotated { expr, annotations } => {
            contains_symbol(expr, namespace, name)
                || annotations
                    .iter()
                    .any(|(_, value)| contains_symbol(value, namespace, name))
        }
        Expr::Extension { payload, .. } => contains_symbol(payload, namespace, name),
        _ => false,
    }
}
