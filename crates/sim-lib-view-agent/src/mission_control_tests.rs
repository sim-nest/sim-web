use sim_kernel::{Expr, Symbol};
use sim_lib_intent::{Origin, intent, validate_intent};

use crate::fake_mission_control_state;
use crate::mission_control::{
    mission_control_intents, mission_control_replay_frames, mission_control_view,
};

#[test]
fn mission_control_renders_fake_fixture_as_scene() {
    let state = fake_mission_control_state().unwrap();
    let scene = mission_control_view(&state);

    sim_lib_scene::validate_scene(&scene).expect("mission control scene validates");
    assert_eq!(state.missions.len(), 1);
    assert_eq!(state.evidence.len(), 5);
    assert_eq!(state.lease_conflicts.len(), 1);
    assert!(scene_text(&scene).contains("F6 attribution"));
    assert!(scene_text(&scene).contains("Approve Mission Control change"));
}

#[test]
fn mission_control_replays_one_frame_per_cassette_step() {
    let state = fake_mission_control_state().unwrap();
    let frames = mission_control_replay_frames(&state);

    assert_eq!(frames.len(), state.evidence.len() + 1);
    for frame in &frames {
        sim_lib_scene::validate_scene(frame).expect("replay frame validates");
    }
}

#[test]
fn lease_conflict_lists_exact_targets() {
    let state = fake_mission_control_state().unwrap();
    let conflict = &state.lease_conflicts[0];

    assert_eq!(conflict.left.target, "sim-lib-view-agent");
    assert_eq!(conflict.right.target, "sim-lib-view-agent");
    assert_eq!(
        conflict.left_mission,
        Symbol::qualified("agent/mission", "mission-control-fixture")
    );
}

#[test]
fn mission_control_intents_validate() {
    let kinds = mission_control_intents()
        .into_iter()
        .map(|intent| intent.kind)
        .collect::<Vec<_>>();

    for expected in [
        "approve",
        "reject",
        "ask",
        "split-mission",
        "pause-agent",
        "rerun-validation",
        "replay-cassette",
        "open-source",
    ] {
        assert!(kinds.contains(&expected), "missing intent {expected}");
    }

    let approve = intent(
        "approve",
        Origin::agent(1),
        vec![("mission", Expr::Symbol(Symbol::new("mission-a")))],
    );
    validate_intent(&approve).expect("approve intent validates");

    let replay = intent(
        "replay-cassette",
        Origin::human(2),
        vec![
            ("mission", Expr::Symbol(Symbol::new("mission-a"))),
            ("at", sim_value::build::uint(3)),
        ],
    );
    validate_intent(&replay).expect("replay intent validates");
}

fn scene_text(expr: &Expr) -> String {
    let mut text = String::new();
    collect_text(expr, &mut text);
    text
}

fn collect_text(expr: &Expr, output: &mut String) {
    match expr {
        Expr::String(value) => {
            output.push_str(value);
            output.push('\n');
        }
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) | Expr::Block(items) => {
            for item in items {
                collect_text(item, output);
            }
        }
        Expr::Map(entries) => {
            for (key, value) in entries {
                collect_text(key, output);
                collect_text(value, output);
            }
        }
        Expr::Call { operator, args } => {
            collect_text(operator, output);
            for arg in args {
                collect_text(arg, output);
            }
        }
        Expr::Infix { left, right, .. } => {
            collect_text(left, output);
            collect_text(right, output);
        }
        Expr::Prefix { arg, .. } | Expr::Postfix { arg, .. } => collect_text(arg, output),
        Expr::Quote { expr, .. } => collect_text(expr, output),
        Expr::Annotated { expr, annotations } => {
            collect_text(expr, output);
            for (_, annotation) in annotations {
                collect_text(annotation, output);
            }
        }
        Expr::Extension { payload, .. } => collect_text(payload, output),
        Expr::Nil
        | Expr::Bool(_)
        | Expr::Number(_)
        | Expr::Symbol(_)
        | Expr::Local(_)
        | Expr::Bytes(_) => {}
    }
}
