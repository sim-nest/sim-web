//! Tests for wrist raw-input reduction into standard Intents.

use sim_kernel::{Expr, Symbol};
use sim_value::build;

use crate::{
    Hit, HitRole, Origin, WristIntentReducer, WristRawInput, field, intent_kind_of, validate_intent,
};

fn input(names: &[&str]) -> Vec<Symbol> {
    names.iter().map(|name| Symbol::new(*name)).collect()
}

fn kind_name(intent: &Expr) -> String {
    intent_kind_of(intent)
        .expect("intent kind")
        .name
        .to_string()
}

#[test]
fn wrist_inputs_reduce_to_standard_intents() {
    let profile = input(&["button", "tap", "touch", "raise"]);
    let mut reducer = WristIntentReducer::new();

    let invoke = reducer
        .reduce(
            Origin::human(1),
            WristRawInput::Tap {
                count: 2,
                target: build::sym("focused"),
                span_ms: 180,
                at_ms: 100,
            },
            &profile,
        )
        .expect("double tap invokes");
    assert_eq!(kind_name(&invoke), "invoke");
    assert_eq!(
        field(&invoke, "op"),
        Some(&Expr::Symbol(Symbol::qualified(
            "watch/input",
            "double-tap"
        )))
    );
    validate_intent(&invoke).expect("double tap yields a valid Intent");

    let dismiss = reducer
        .reduce(
            Origin::human(2),
            WristRawInput::Button {
                id: Symbol::new("back"),
                held_ms: 900,
                at_ms: 300,
            },
            &profile,
        )
        .expect("long press dismisses");
    assert_eq!(kind_name(&dismiss), "dismiss");
    validate_intent(&dismiss).expect("long press yields a valid Intent");

    let touch = reducer
        .reduce(
            Origin::human(3),
            WristRawInput::Touch {
                hit: Hit::on(HitRole::Node, build::sym("row-2")),
                at_ms: 500,
            },
            &profile,
        )
        .expect("touch moves selection");
    assert_eq!(kind_name(&touch), "move");
    assert_eq!(field(&touch, "node"), Some(&build::sym("row-2")));
    validate_intent(&touch).expect("touch yields a valid Intent");

    let edit = reducer
        .reduce(
            Origin::human(4),
            WristRawInput::Tap {
                count: 3,
                target: build::sym("focused"),
                span_ms: 260,
                at_ms: 700,
            },
            &profile,
        )
        .expect("triple tap edits");
    assert_eq!(kind_name(&edit), "edit");
    validate_intent(&edit).expect("triple tap yields a valid Intent");
}

#[test]
fn crown_rotation_requires_a_profile_capability() {
    let target = build::sym("focused");
    let mut trex = WristIntentReducer::new();
    let no_crown = input(&["button", "tap", "touch", "raise"]);

    assert!(
        trex.reduce(
            Origin::human(1),
            WristRawInput::Crown {
                delta: 1,
                press: false,
                target: target.clone(),
                at_ms: 100,
            },
            &no_crown,
        )
        .is_none()
    );

    let with_crown = input(&["button", "tap", "touch", "raise", "crown"]);
    let mut crown = WristIntentReducer::new();
    let moved = crown
        .reduce(
            Origin::human(1),
            WristRawInput::Crown {
                delta: -2,
                press: false,
                target,
                at_ms: 100,
            },
            &with_crown,
        )
        .expect("crown-capable profile moves selection");
    assert_eq!(kind_name(&moved), "move");
    validate_intent(&moved).expect("crown yields a valid Intent");
}

#[test]
fn wrist_jitter_and_debounce_fire_nothing() {
    let profile = input(&["button", "tap", "touch", "raise"]);
    let mut reducer = WristIntentReducer::new();

    assert!(
        reducer
            .reduce(
                Origin::human(1),
                WristRawInput::Raise {
                    target: build::sym("focused"),
                    stable_ms: 20,
                    at_ms: 100,
                },
                &profile,
            )
            .is_none(),
        "short raise jitter must not become an Intent"
    );

    assert!(
        reducer
            .reduce(
                Origin::human(2),
                WristRawInput::Tap {
                    count: 1,
                    target: build::sym("focused"),
                    span_ms: 80,
                    at_ms: 200,
                },
                &profile,
            )
            .is_some()
    );
    assert!(
        reducer
            .reduce(
                Origin::human(3),
                WristRawInput::Tap {
                    count: 2,
                    target: build::sym("focused"),
                    span_ms: 90,
                    at_ms: 230,
                },
                &profile,
            )
            .is_none(),
        "events inside the debounce window must not fire"
    );
}
