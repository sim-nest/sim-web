//! Tests for the Intent value model, `codec:intent`, and the gesture algebra.

use std::sync::Arc;

use sim_codec::{Input, Output, decode_with_codec, encode_with_codec};
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, EncodeOptions, Expr, ReadPolicy, Symbol};

use crate::gesture::{
    GestureRecognizer, Hit, HitRole, PointerEvent, PointerPhase, intent_from_gesture,
};
use crate::{
    INTENT_KINDS, IntentCodecLib, Origin, intent, intent_codec_symbol, intent_kind_of,
    intent_shape_specs, intent_shape_symbol, referenced_targets, required_fields, resolve_targets,
    validate_intent,
};

fn cx() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::register_core_classes(&mut cx);
    let lib = IntentCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&lib).unwrap();
    cx
}

use sim_value::build::sym;

/// Build a structurally valid Intent of `kind` by filling every required field.
fn sample_intent(kind: &str) -> Expr {
    let fields = required_fields(kind)
        .iter()
        .map(|name| {
            let value = if *name == "path" {
                Expr::List(vec![Expr::Vector(vec![sym("k"), sym("title")])])
            } else {
                sym(&format!("{name}-value"))
            };
            (*name, value)
        })
        .collect();
    intent(kind, Origin::human(7), fields)
}

#[test]
fn every_intent_kind_roundtrips_through_codec_intent() {
    let mut cx = cx();
    let codec = intent_codec_symbol();
    for kind in INTENT_KINDS {
        let value = sample_intent(kind);
        validate_intent(&value).unwrap_or_else(|err| panic!("sample {kind} invalid: {err}"));
        let output = encode_with_codec(&mut cx, &codec, &value, EncodeOptions::default()).unwrap();
        let input = match output {
            Output::Text(text) => Input::Text(text),
            Output::Bytes(bytes) => Input::Bytes(bytes),
        };
        let decoded = decode_with_codec(&mut cx, &codec, input, ReadPolicy::default()).unwrap();
        assert_eq!(value, decoded, "kind {kind} must round-trip");
    }
}

#[test]
fn validation_rejects_structural_problems() {
    // missing kind
    assert!(validate_intent(&Expr::Map(vec![])).is_err());
    // unknown kind
    let unknown = Expr::Map(vec![(
        sym("kind"),
        Expr::Symbol(Symbol::qualified("intent", "nope")),
    )]);
    assert!(validate_intent(&unknown).is_err());
    // missing origin
    let no_origin = Expr::Map(vec![(
        sym("kind"),
        Expr::Symbol(Symbol::qualified("intent", "commit")),
    )]);
    assert!(validate_intent(&no_origin).is_err());
    // bad operator
    let bad_op = Expr::Map(vec![
        (
            sym("kind"),
            Expr::Symbol(Symbol::qualified("intent", "commit")),
        ),
        (
            sym("origin"),
            Expr::Map(vec![
                (sym("operator"), sym("robot")),
                (sym("at-tick"), sym("x")),
            ]),
        ),
        (sym("pane"), sym("p")),
    ]);
    assert!(validate_intent(&bad_op).is_err());
    // missing required field
    let missing = intent("wire", Origin::human(1), vec![("from", sym("a"))]);
    assert!(validate_intent(&missing).is_err());
    // edit-field path not a list
    let bad_path = intent(
        "edit-field",
        Origin::human(1),
        vec![
            ("target", sym("t")),
            ("path", sym("not-a-list")),
            ("value", sym("v")),
        ],
    );
    assert!(validate_intent(&bad_path).is_err());
}

#[test]
fn kind_shapes_reject_wrong_kind_and_keep_dispatch_scores() {
    let mut cx = cx();
    let wire = sample_intent("wire");
    let move_node = sample_intent("move");
    let wire_shape = intent_shape("Wire");
    let umbrella = intent_shape_symbol_shape();

    let wire_match = wire_shape.check_expr(&mut cx, &wire).unwrap();
    assert!(wire_match.accepted);
    assert_eq!(wire_match.score.value(), 20);

    assert!(!wire_shape.check_expr(&mut cx, &move_node).unwrap().accepted);

    let umbrella_match = umbrella.check_expr(&mut cx, &move_node).unwrap();
    assert!(umbrella_match.accepted);
    assert_eq!(umbrella_match.score.value(), 5);

    let unknown = intent("not-a-real-kind", Origin::human(1), vec![]);
    assert!(!umbrella.check_expr(&mut cx, &unknown).unwrap().accepted);
}

#[test]
fn unknown_target_yields_a_diagnostic_not_a_mutation() {
    let wire = intent(
        "wire",
        Origin::human(1),
        vec![
            (
                "from",
                Expr::Map(vec![(sym("node"), sym("n1")), (sym("port"), sym("out0"))]),
            ),
            (
                "to",
                Expr::Map(vec![
                    (sym("node"), sym("missing")),
                    (sym("port"), sym("in0")),
                ]),
            ),
        ],
    );
    let known = |target: &Expr| matches!(target, Expr::Symbol(s) if &*s.name == "n1");
    let error = resolve_targets(&wire, known).expect_err("unknown target must error");
    assert!(
        error.path_string().contains("to.node"),
        "path: {}",
        error.path_string()
    );

    // When every target resolves, no diagnostic is produced.
    let everything = |_: &Expr| true;
    assert!(resolve_targets(&wire, everything).is_ok());
}

#[test]
fn referenced_targets_follow_the_kind() {
    let select = intent(
        "select",
        Origin::human(1),
        vec![("targets", Expr::List(vec![sym("a"), sym("b")]))],
    );
    assert_eq!(referenced_targets(&select).len(), 2);
    let commit = intent("commit", Origin::human(1), vec![("pane", sym("p"))]);
    assert!(referenced_targets(&commit).is_empty());
}

#[test]
fn performance_event_intent_targets_a_bound_source() {
    let event = Expr::Map(vec![
        (
            sym("kind"),
            Expr::Symbol(Symbol::qualified("music/performance-intent", "note-on")),
        ),
        (sym("pitch"), Expr::String("60".to_owned())),
        (sym("velocity"), Expr::String("96".to_owned())),
        (sym("channel"), Expr::String("0".to_owned())),
    ]);
    let target = Expr::Symbol(Symbol::qualified("music/performance-source", "keyboard"));
    let value = intent(
        "performance-event",
        Origin::human(9),
        vec![
            ("target", target.clone()),
            (
                "source",
                Expr::Symbol(Symbol::qualified("music/performance-source", "keyboard")),
            ),
            (
                "input",
                Expr::Symbol(Symbol::qualified("midi/input", "keyboard")),
            ),
            ("event", event),
        ],
    );
    validate_intent(&value).expect("performance event intent validates");
    assert_eq!(
        referenced_targets(&value),
        vec![("target".to_owned(), target)]
    );
}

#[test]
fn music_editor_intents_target_roll_and_rack() {
    for kind in ["piano-roll-edit", "player-rack-edit", "arranger-edit"] {
        let target = Expr::Symbol(Symbol::qualified("music/editor", kind));
        let value = intent(
            kind,
            Origin::human(10),
            vec![("target", target.clone()), ("action", sym("freeze"))],
        );
        validate_intent(&value).unwrap_or_else(|err| panic!("{kind}: {err}"));
        assert_eq!(
            referenced_targets(&value),
            vec![("target".to_owned(), target)]
        );
    }
}

#[test]
fn mission_control_intents_require_mission_or_location() {
    for kind in ["approve", "reject", "pause-agent", "rerun-validation"] {
        let value = intent(kind, Origin::agent(1), vec![("mission", sym("m"))]);
        validate_intent(&value).unwrap_or_else(|err| panic!("{kind}: {err}"));
        assert_eq!(
            referenced_targets(&value),
            vec![("mission".to_owned(), sym("m"))]
        );
    }

    let ask = intent(
        "ask",
        Origin::agent(2),
        vec![
            ("mission", sym("m")),
            ("question", Expr::String("Proceed?".to_owned())),
        ],
    );
    validate_intent(&ask).expect("ask intent validates");

    let split = intent(
        "split-mission",
        Origin::agent(3),
        vec![
            ("mission", sym("m")),
            ("goals", Expr::List(vec![sym("a"), sym("b")])),
        ],
    );
    validate_intent(&split).expect("split-mission intent validates");

    let replay = intent(
        "replay-cassette",
        Origin::human(4),
        vec![("mission", sym("m")), ("at", sim_value::build::uint(2))],
    );
    validate_intent(&replay).expect("replay intent validates");

    let open = intent(
        "open-source",
        Origin::human(5),
        vec![("location", sym("span"))],
    );
    validate_intent(&open).expect("open-source intent validates");
    assert_eq!(
        referenced_targets(&open),
        vec![("location".to_owned(), sym("span"))]
    );
}

#[test]
fn recognizer_folds_pointer_stream_into_a_tap() {
    let mut recognizer = GestureRecognizer::new();
    let hit = Hit::on(HitRole::Node, sym("n1"));
    assert!(recognizer.pointer(down(10.0, 10.0, hit.clone())).is_none());
    let gesture = recognizer
        .pointer(up(11.0, 11.0, hit.clone()))
        .expect("release completes a gesture");
    let value = intent_from_gesture(Origin::human(3), "pane-graph", &gesture).unwrap();
    assert_eq!(
        intent_kind_of(&value).map(|symbol| symbol.name.to_string()),
        Some("select".to_owned())
    );
}

#[test]
fn dragging_between_ports_wires_them() {
    let mut recognizer = GestureRecognizer::new();
    let from = Hit::on(HitRole::Port, sym("n1.out0"))
        .with("node", sym("n1"))
        .with("port", sym("out0"));
    let to = Hit::on(HitRole::Port, sym("n2.in0"))
        .with("node", sym("n2"))
        .with("port", sym("in0"));
    recognizer.pointer(down(0.0, 0.0, from));
    recognizer.pointer(event(PointerPhase::Move, 40.0, 0.0, Hit::blank()));
    let gesture = recognizer.pointer(up(80.0, 0.0, to)).expect("a drag");
    let value = intent_from_gesture(Origin::human(1), "pane-graph", &gesture).unwrap();
    assert_eq!(
        intent_kind_of(&value).map(|symbol| symbol.name.to_string()),
        Some("wire".to_owned())
    );
    validate_intent(&value).expect("a composed wire intent must validate");
}

#[test]
fn dragging_a_node_moves_it() {
    let mut recognizer = GestureRecognizer::new();
    let node = Hit::on(HitRole::Node, sym("n1"));
    recognizer.pointer(down(0.0, 0.0, node));
    recognizer.pointer(event(PointerPhase::Move, 50.0, 50.0, Hit::blank()));
    let gesture = recognizer
        .pointer(up(60.0, 70.0, Hit::blank()))
        .expect("a drag");
    let value = intent_from_gesture(Origin::human(1), "pane-graph", &gesture).unwrap();
    assert_eq!(
        intent_kind_of(&value).map(|symbol| symbol.name.to_string()),
        Some("move".to_owned())
    );
}

#[test]
fn key_commands_map_to_intents() {
    let node = Hit::on(HitRole::Node, sym("n1"));
    let delete = GestureRecognizer::key("delete", node);
    let value = intent_from_gesture(Origin::agent(9), "pane-graph", &delete).unwrap();
    assert_eq!(
        intent_kind_of(&value).map(|symbol| symbol.name.to_string()),
        Some("delete".to_owned())
    );

    let commit = GestureRecognizer::key("commit", Hit::blank());
    let value = intent_from_gesture(Origin::human(1), "pane-graph", &commit).unwrap();
    assert_eq!(
        intent_kind_of(&value).map(|symbol| symbol.name.to_string()),
        Some("commit".to_owned())
    );
}

#[test]
fn meaningless_gestures_fail_closed() {
    // A button tap with no control detail has no meaning.
    let button = Hit::on(HitRole::Button, sym("b"));
    let tap = crate::gesture::RawGesture::Tap { hit: button };
    assert!(intent_from_gesture(Origin::human(1), "p", &tap).is_err());

    // A drag from blank to blank has no meaning.
    let drag = crate::gesture::RawGesture::Drag {
        from: Hit::blank(),
        to: Hit::blank(),
        at: (1.0, 1.0),
    };
    assert!(intent_from_gesture(Origin::human(1), "p", &drag).is_err());
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

fn intent_shape(name: &str) -> Arc<dyn sim_kernel::Shape> {
    let symbol = Symbol::qualified("intent", name);
    shape_by_symbol(symbol)
}

fn intent_shape_symbol_shape() -> Arc<dyn sim_kernel::Shape> {
    shape_by_symbol(intent_shape_symbol())
}

fn shape_by_symbol(symbol: Symbol) -> Arc<dyn sim_kernel::Shape> {
    intent_shape_specs()
        .into_iter()
        .find(|(candidate, _)| candidate == &symbol)
        .map(|(_, shape)| shape)
        .unwrap_or_else(|| panic!("missing Intent shape {symbol}"))
}
