//! Tests for glasses raw-input reduction into standard Intents.

use sim_kernel::{Expr, Symbol};
use sim_value::build;

use crate::{
    ControllerAction, GazePhase, GlassesIntentReducer, GlassesRawInput, HeadGesture, Hit, HitRole,
    Origin, field, intent_kind_of, validate_intent,
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
fn glasses_gestures_reduce_to_standard_intents() {
    let profile = input(&["gaze", "head", "hand", "tap", "button", "controller"]);
    let mut reducer = GlassesIntentReducer::new();

    let gaze = reducer
        .reduce(
            Origin::human(1),
            GlassesRawInput::Gaze {
                phase: GazePhase::Dwell,
                hit: Hit::on(HitRole::Node, build::sym("panel-a")),
                stable_ms: 700,
                vio_stable: true,
                at_ms: 100,
            },
            &profile,
        )
        .expect("gaze dwell selects");
    assert_eq!(kind_name(&gaze), "select");
    assert_eq!(
        field(&gaze, "targets"),
        Some(&Expr::List(vec![build::sym("panel-a")]))
    );
    validate_intent(&gaze).expect("gaze dwell yields a valid Intent");

    let pinch = reducer
        .reduce(
            Origin::human(2),
            GlassesRawInput::Pinch {
                hit: Hit::on(HitRole::Button, build::sym("primary-action")),
                stable_ms: 100,
                vio_stable: true,
                at_ms: 300,
            },
            &profile,
        )
        .expect("pinch invokes");
    assert_eq!(kind_name(&pinch), "invoke");
    assert_eq!(field(&pinch, "target"), Some(&build::sym("primary-action")));
    assert_eq!(
        field(&pinch, "op"),
        Some(&Expr::Symbol(Symbol::qualified("glasses/input", "pinch")))
    );
    validate_intent(&pinch).expect("pinch yields a valid Intent");

    let double_tap = reducer
        .reduce(
            Origin::human(3),
            GlassesRawInput::Tap {
                count: 2,
                target: build::sym("focused-card"),
                span_ms: 220,
                held_ms: 40,
                at_ms: 500,
            },
            &profile,
        )
        .expect("double tap invokes");
    assert_eq!(kind_name(&double_tap), "invoke");
    assert_eq!(
        field(&double_tap, "op"),
        Some(&Expr::Symbol(Symbol::qualified(
            "glasses/input",
            "double-tap"
        )))
    );
    validate_intent(&double_tap).expect("double tap yields a valid Intent");
}

#[test]
fn glasses_reducer_maps_the_full_non_voice_set() {
    let profile = input(&["gaze", "head", "hand", "tap", "button", "controller"]);
    let mut reducer = GlassesIntentReducer::new();

    let gaze_enter = reducer
        .reduce(
            Origin::human(1),
            GlassesRawInput::Gaze {
                phase: GazePhase::Enter,
                hit: Hit::on(HitRole::Node, build::sym("panel-b")),
                stable_ms: 150,
                vio_stable: true,
                at_ms: 100,
            },
            &profile,
        )
        .expect("gaze enter moves focus");
    assert_eq!(kind_name(&gaze_enter), "move");
    validate_intent(&gaze_enter).expect("gaze enter yields a valid Intent");

    let head_nod = reducer
        .reduce(
            Origin::human(2),
            GlassesRawInput::Head {
                kind: HeadGesture::Nod,
                target: build::sym("focused-card"),
                stable_ms: 160,
                vio_stable: true,
                at_ms: 300,
            },
            &profile,
        )
        .expect("head nod invokes");
    assert_eq!(kind_name(&head_nod), "invoke");
    validate_intent(&head_nod).expect("head nod yields a valid Intent");

    let head_shake = reducer
        .reduce(
            Origin::human(3),
            GlassesRawInput::Head {
                kind: HeadGesture::Shake,
                target: build::sym("focused-card"),
                stable_ms: 160,
                vio_stable: true,
                at_ms: 500,
            },
            &profile,
        )
        .expect("head shake dismisses");
    assert_eq!(kind_name(&head_shake), "dismiss");
    validate_intent(&head_shake).expect("head shake yields a valid Intent");

    let hand_ray = reducer
        .reduce(
            Origin::human(4),
            GlassesRawInput::HandRay {
                hit: Hit::on(HitRole::Node, build::sym("panel-c")),
                stable_ms: 120,
                vio_stable: true,
                at_ms: 700,
            },
            &profile,
        )
        .expect("hand ray moves focus");
    assert_eq!(kind_name(&hand_ray), "move");
    validate_intent(&hand_ray).expect("hand ray yields a valid Intent");

    let single_tap = reducer
        .reduce(
            Origin::human(5),
            GlassesRawInput::Tap {
                count: 1,
                target: build::sym("focused-card"),
                span_ms: 120,
                held_ms: 40,
                at_ms: 900,
            },
            &profile,
        )
        .expect("single tap selects");
    assert_eq!(kind_name(&single_tap), "select");
    validate_intent(&single_tap).expect("single tap yields a valid Intent");

    let long_tap = reducer
        .reduce(
            Origin::human(6),
            GlassesRawInput::Tap {
                count: 1,
                target: build::sym("focused-card"),
                span_ms: 700,
                held_ms: 700,
                at_ms: 1100,
            },
            &profile,
        )
        .expect("long tap edits");
    assert_eq!(kind_name(&long_tap), "edit");
    validate_intent(&long_tap).expect("long tap yields a valid Intent");

    let button = reducer
        .reduce(
            Origin::human(7),
            GlassesRawInput::Button {
                id: Symbol::new("front"),
                target: None,
                held_ms: 100,
                at_ms: 1300,
            },
            &profile,
        )
        .expect("button invokes");
    assert_eq!(kind_name(&button), "invoke");
    assert_eq!(field(&button, "target"), Some(&build::sym("front")));
    validate_intent(&button).expect("button yields a valid Intent");

    let controller_move = reducer
        .reduce(
            Origin::human(8),
            GlassesRawInput::Controller {
                id: Symbol::new("ring"),
                action: ControllerAction::Move { dx: 1, dy: 0 },
                target: build::sym("focused-card"),
                at_ms: 1500,
            },
            &profile,
        )
        .expect("controller moves");
    assert_eq!(kind_name(&controller_move), "move");
    validate_intent(&controller_move).expect("controller move yields a valid Intent");
}

#[test]
fn glasses_jitter_unstable_vio_and_voice_cap_fire_nothing() {
    let profile = input(&["gaze", "head", "hand", "tap", "button", "controller"]);
    let mut reducer = GlassesIntentReducer::new();

    assert!(
        reducer
            .reduce(
                Origin::human(1),
                GlassesRawInput::Gaze {
                    phase: GazePhase::Dwell,
                    hit: Hit::on(HitRole::Node, build::sym("panel-a")),
                    stable_ms: 120,
                    vio_stable: true,
                    at_ms: 100,
                },
                &profile,
            )
            .is_none(),
        "short dwell jitter must not select"
    );

    assert!(
        reducer
            .reduce(
                Origin::human(2),
                GlassesRawInput::Pinch {
                    hit: Hit::on(HitRole::Button, build::sym("primary-action")),
                    stable_ms: 100,
                    vio_stable: false,
                    at_ms: 300,
                },
                &profile,
            )
            .is_none(),
        "unstable VIO must not invoke"
    );

    assert!(
        reducer
            .reduce(
                Origin::human(3),
                GlassesRawInput::Tap {
                    count: 2,
                    target: build::sym("focused-card"),
                    span_ms: 900,
                    held_ms: 20,
                    at_ms: 500,
                },
                &profile,
            )
            .is_none(),
        "overlong double tap must not invoke"
    );

    let voice_only = input(&["voice"]);
    assert!(
        reducer
            .reduce(
                Origin::human(4),
                GlassesRawInput::Tap {
                    count: 2,
                    target: build::sym("focused-card"),
                    span_ms: 200,
                    held_ms: 20,
                    at_ms: 700,
                },
                &voice_only,
            )
            .is_none(),
        "voice capability is not a raw glasses Intent route"
    );

    let first = reducer.reduce(
        Origin::human(5),
        GlassesRawInput::Tap {
            count: 1,
            target: build::sym("focused-card"),
            span_ms: 90,
            held_ms: 20,
            at_ms: 900,
        },
        &profile,
    );
    assert!(first.is_some());

    let repeated = reducer.reduce(
        Origin::human(6),
        GlassesRawInput::Tap {
            count: 2,
            target: build::sym("focused-card"),
            span_ms: 90,
            held_ms: 20,
            at_ms: 930,
        },
        &profile,
    );
    assert!(
        repeated.is_none(),
        "events inside the debounce window must not fire"
    );
}
