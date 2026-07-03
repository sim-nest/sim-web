//! Tests for on-screen performance keyboard descriptors.

use sim_kernel::{Expr, Symbol};

use crate::keyboard::{
    PERFORMANCE_KEYBOARD_VIEW_ID, PerformanceKeyAction, PerformanceKeyMap,
    PerformanceKeyboardBinding, PerformanceKeyboardState, performance_keyboard_demo_scene,
    performance_keyboard_view,
};

use sim_value::build::sym;

#[test]
fn keyboard_scene_describes_playable_controls_and_binding() {
    let binding = PerformanceKeyboardBinding::browser(
        Symbol::qualified("music/player-chain", "lead"),
        Symbol::qualified("audio-synth/instrument", "dx7"),
    );
    let state = PerformanceKeyboardState {
        base_midi: 60,
        octaves: 2,
        held_notes: vec![60],
        generated_notes: vec![67],
        sustain: true,
        ..PerformanceKeyboardState::default()
    };
    let scene = performance_keyboard_view(&binding, &state);

    sim_lib_scene::validate_scene(&scene).expect("keyboard scene is valid");
    assert!(sim_test_support::contains_kind(&scene, "keyboard"));
    assert_eq!(
        field(&scene, "lens"),
        Some(sym(PERFORMANCE_KEYBOARD_VIEW_ID))
    );
    assert_eq!(field(&scene, "role"), Some(sym("performance-keyboard")));
    assert_eq!(
        nested_field(&scene, "binding", "player-chain"),
        Some(Expr::Symbol(Symbol::qualified(
            "music/player-chain",
            "lead"
        )))
    );
    assert_eq!(
        nested_field(&scene, "binding", "instrument"),
        Some(Expr::Symbol(Symbol::qualified(
            "audio-synth/instrument",
            "dx7"
        )))
    );

    let keys = field(&scene, "keys").expect("keys");
    let Expr::List(keys) = keys else {
        panic!("keys list")
    };
    assert_eq!(keys.len(), 24);
    assert!(key_flag(&keys, 60, "held"));
    assert!(key_flag(&keys, 67, "generated"));
    assert!(key_flag(&keys, 64, "scale"));
    assert!(!key_flag(&keys, 61, "scale"));

    assert_eq!(
        nested_field(&scene, "key-map", "editable"),
        Some(Expr::Bool(true))
    );
    assert_eq!(
        nested_field(&scene, "key-map", "name"),
        Some(Expr::String("qwerty-two-row".to_owned()))
    );
}

#[test]
fn keyboard_demo_binds_chain_instrument_and_stream() {
    let scene = performance_keyboard_demo_scene();
    sim_lib_scene::validate_scene(&scene).expect("keyboard demo scene is valid");
    assert!(sim_test_support::contains_kind(&scene, "keyboard"));
    assert_eq!(
        nested_field(&scene, "binding", "instrument"),
        Some(Expr::Symbol(Symbol::qualified(
            "audio-synth/instrument",
            "dx7"
        )))
    );
    assert_eq!(
        nested_field(&scene, "binding", "stream"),
        Some(Expr::Symbol(Symbol::qualified(
            "stream/browser",
            "performance-keyboard"
        )))
    );
}

#[test]
fn default_key_map_serializes_rows_and_controls() {
    let key_map = PerformanceKeyMap::default();
    assert_eq!(key_map.name, "qwerty-two-row");
    assert!(key_map.editable);
    assert!(key_map.entries.iter().any(|entry| {
        entry.code == "KeyZ"
            && entry.action
                == PerformanceKeyAction::Degree {
                    degree: 0,
                    octave: 0,
                }
    }));
    assert!(key_map.entries.iter().any(|entry| {
        entry.code == "KeyQ"
            && entry.action
                == PerformanceKeyAction::Degree {
                    degree: 0,
                    octave: 1,
                }
    }));
    assert!(
        key_map
            .entries
            .iter()
            .any(|entry| entry.code == "Space" && entry.action == PerformanceKeyAction::Sustain)
    );
    assert!(
        key_map
            .entries
            .iter()
            .any(|entry| entry.code == "Escape" && entry.action == PerformanceKeyAction::Panic)
    );
    assert!(key_map.entries.iter().any(|entry| {
        entry.code == "F4" && entry.action == PerformanceKeyAction::Velocity { value: 127 }
    }));

    let binding = PerformanceKeyboardBinding::browser(
        Symbol::qualified("music/player-chain", "lead"),
        Symbol::qualified("audio-synth/instrument", "dx7"),
    );
    let scene = performance_keyboard_view(&binding, &PerformanceKeyboardState::default());
    let entries = nested_field(&scene, "key-map", "entries").expect("key-map entries");
    let Expr::List(entries) = entries else {
        panic!("entries list")
    };
    assert!(entry_with_action(&entries, "KeyZ", "degree"));
    assert!(entry_with_action(&entries, "BracketRight", "octave-shift"));
    assert!(entry_with_action(&entries, "Backslash", "scale-lock"));
}

fn field(map: &Expr, name: &str) -> Option<Expr> {
    let Expr::Map(entries) = map else {
        return None;
    };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(s) if &*s.name == name).then(|| value.clone())
    })
}

fn nested_field(map: &Expr, outer: &str, inner: &str) -> Option<Expr> {
    field(map, outer).and_then(|value| field(&value, inner))
}

fn key_flag(keys: &[Expr], midi: i32, flag: &str) -> bool {
    keys.iter().any(|key| {
        matches!(
            field(key, "midi"),
            Some(Expr::Number(number)) if number.canonical == midi.to_string()
        ) && field(key, flag) == Some(Expr::Bool(true))
    })
}

fn entry_with_action(entries: &[Expr], code: &str, action: &str) -> bool {
    entries.iter().any(|entry| {
        field(entry, "code") == Some(Expr::String(code.to_owned()))
            && field(entry, "action") == Some(Expr::String(action.to_owned()))
    })
}
