use sim_kernel::Expr;

use crate::{change_capsule_replay_frames, change_capsule_view, fake_change_capsule_state};

#[test]
fn change_capsule_view_renders_review_panels() {
    let state = fake_change_capsule_state();
    let scene = change_capsule_view(&state);

    sim_lib_scene::validate_scene(&scene).expect("change capsule scene validates");
    let text = scene_text(&scene);
    assert!(text.contains("capsule-diff"));
    assert!(text.contains("capsule-logs"));
    assert!(text.contains("generated-docs"));
    assert!(text.contains("pin-plan"));
    assert!(text.contains("F6 trade-off"));
    assert!(text.contains("replay hash"));
}

#[test]
fn change_capsule_replay_frames_match_cassette_events() {
    let state = fake_change_capsule_state();
    let frames = change_capsule_replay_frames(&state);

    assert_eq!(frames.len(), state.replay.events.len() + 1);
    for frame in &frames {
        sim_lib_scene::validate_scene(frame).expect("replay frame validates");
    }
}

#[test]
fn pin_plan_requires_pushed_commit_flag() {
    let state = fake_change_capsule_state();
    assert!(state.pin_plan[0].pushed_commit_exists);
    assert_eq!(state.pin_plan[0].repo, "sim-agent-net");
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
        Expr::Symbol(symbol) => {
            output.push_str(&symbol.to_string());
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
        Expr::Nil | Expr::Bool(_) | Expr::Number(_) | Expr::Local(_) | Expr::Bytes(_) => {}
    }
}
