//! Tests for the Scene value model, `codec:scene`, and scene diff/apply.

use std::sync::Arc;

use sim_codec::{Input, Output, decode_with_codec, encode_with_codec};
use sim_kernel::{
    Cx, DefaultFactory, EagerPolicy, EncodeOptions, Expr, NumberLiteral, ReadPolicy, Symbol,
};

use crate::{
    GlanceAction, GlanceCard, GlanceMetric, SceneCodecLib, apply, diff, map, node,
    scene_codec_symbol, scene_shape_specs, scene_shape_symbol, text, validate_scene,
};

fn cx() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::register_core_classes(&mut cx);
    let lib = SceneCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&lib).unwrap();
    cx
}

fn num(domain: &str, canonical: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::new(domain),
        canonical: canonical.to_owned(),
    })
}

use sim_value::build::sym;

/// A representative scene exercising every atom and container kind.
fn sample_scene() -> Expr {
    node(
        "graph",
        vec![
            ("id", sym("graph-main")),
            (
                "bounds",
                map(vec![("w", num("i64", "1200")), ("h", num("i64", "700"))]),
            ),
            (
                "nodes",
                Expr::List(vec![
                    node(
                        "node",
                        vec![
                            ("id", sym("n1")),
                            ("title", Expr::String("Planner".to_owned())),
                            (
                                "at",
                                map(vec![("x", num("f64", "80")), ("y", num("f64", "120"))]),
                            ),
                            ("status", sym("ok")),
                            (
                                "target",
                                Expr::List(vec![sym("ref"), sym("agent"), sym("planner")]),
                            ),
                        ],
                    ),
                    node(
                        "node",
                        vec![
                            ("id", sym("n2")),
                            ("title", Expr::String("Writer".to_owned())),
                        ],
                    ),
                ]),
            ),
            ("flags", Expr::Set(vec![sym("a"), sym("b")])),
            ("ports", Expr::Vector(vec![sym("in0"), sym("out0")])),
            ("blob", Expr::Bytes(vec![0, 1, 2, 255, 16])),
            (
                "note",
                Expr::String("quote \" and \\ and \n newline".to_owned()),
            ),
            ("nothing", Expr::Nil),
            ("live", Expr::Bool(true)),
            ("dead", Expr::Bool(false)),
        ],
    )
}

#[test]
fn text_form_roundtrips_losslessly() {
    let scene = sample_scene();
    let encoded = text::encode(sim_kernel::CodecId(7), &scene).unwrap();
    let decoded = text::decode(sim_kernel::CodecId(7), &encoded).unwrap();
    assert_eq!(scene, decoded);
}

#[test]
fn scene_roundtrips_through_codec_scene() {
    let mut cx = cx();
    let codec = scene_codec_symbol();
    let scene = sample_scene();
    let output = encode_with_codec(&mut cx, &codec, &scene, EncodeOptions::default()).unwrap();
    let input = match output {
        Output::Text(text) => Input::Text(text),
        Output::Bytes(bytes) => Input::Bytes(bytes),
    };
    let decoded = decode_with_codec(&mut cx, &codec, input, ReadPolicy::default()).unwrap();
    assert_eq!(scene, decoded);
}

#[test]
fn encoding_a_non_scene_fails_closed() {
    let mut cx = cx();
    let codec = scene_codec_symbol();
    // A map with no kind tag is not a scene node.
    let not_a_scene = map(vec![("just", sym("data"))]);
    let err = encode_with_codec(&mut cx, &codec, &not_a_scene, EncodeOptions::default());
    assert!(err.is_err(), "a kindless map must not encode as a scene");
}

#[test]
fn encoding_a_non_data_form_fails_closed() {
    let mut cx = cx();
    let codec = scene_codec_symbol();
    // A scene carrying an eval-only Call form is not pure data.
    let scene = node(
        "box",
        vec![(
            "bad",
            Expr::Call {
                operator: Box::new(sym("f")),
                args: vec![sym("x")],
            },
        )],
    );
    let err = encode_with_codec(&mut cx, &codec, &scene, EncodeOptions::default());
    assert!(err.is_err(), "non-data forms must not encode as a scene");
}

#[test]
fn decoding_malformed_text_yields_a_diagnostic_not_a_panic() {
    let mut cx = cx();
    let codec = scene_codec_symbol();
    for bad in [
        "",
        "(",
        "{ S U\"k\" }",
        "Znonsense",
        "%(",
        "R\"unterminated",
    ] {
        let result = decode_with_codec(
            &mut cx,
            &codec,
            Input::Text(bad.to_owned()),
            ReadPolicy::default(),
        );
        assert!(
            result.is_err(),
            "malformed input {bad:?} must error, not panic"
        );
    }
}

#[test]
fn validate_reports_a_structured_path() {
    // A nested node with an unrecognized kind reports its address.
    let scene = node(
        "graph",
        vec![("nodes", Expr::List(vec![node("not-a-real-kind", vec![])]))],
    );
    let error = validate_scene(&scene).expect_err("must reject unknown nested kind");
    assert!(
        error.path_string().contains("nodes"),
        "path: {}",
        error.path_string()
    );
    assert!(error.message.contains("unrecognized scene kind"));
}

#[test]
fn validate_rejects_kindless_and_non_symbol_kinds() {
    assert!(validate_scene(&map(vec![("x", sym("y"))])).is_err());
    let bad_kind = Expr::Map(vec![(sym("kind"), Expr::String("graph".to_owned()))]);
    assert!(validate_scene(&bad_kind).is_err());
}

#[test]
fn kind_shapes_reject_wrong_kind_and_keep_dispatch_scores() {
    let mut cx = cx();
    let graph = node("graph", vec![("id", sym("graph-main"))]);
    let box_node = node("box", vec![("id", sym("box-main"))]);
    let graph_shape = scene_shape("Graph");
    let umbrella = scene_shape_symbol_shape();

    let graph_match = graph_shape.check_expr(&mut cx, &graph).unwrap();
    assert!(graph_match.accepted);
    assert_eq!(graph_match.score.value(), 20);

    assert!(!graph_shape.check_expr(&mut cx, &box_node).unwrap().accepted);

    let umbrella_match = umbrella.check_expr(&mut cx, &box_node).unwrap();
    assert!(umbrella_match.accepted);
    assert_eq!(umbrella_match.score.value(), 5);

    let unknown = node("not-a-real-kind", vec![]);
    assert!(!umbrella.check_expr(&mut cx, &unknown).unwrap().accepted);
}

#[test]
fn validates_music_editor_scene_kinds() {
    for kind in ["piano-roll", "player-rack", "object-roll"] {
        validate_scene(&node(kind, vec![("target", sym("target"))]))
            .unwrap_or_else(|err| panic!("{kind}: {err}"));
    }
}

#[test]
fn glance_card_is_a_scene_kind() {
    let card = GlanceCard::new(
        "Drive",
        Some(GlanceMetric::new("speed", "42")),
        Some(GlanceAction::new("Ack", sym("ack"))),
        "info",
        4,
    )
    .to_scene();

    validate_scene(&card).expect("scene/glance validates");
    let parsed = GlanceCard::from_scene(&card).expect("glance parses");

    assert_eq!(parsed.title, "Drive");
    assert_eq!(parsed.metric.unwrap().value, "42");
    assert_eq!(parsed.action.unwrap().label, "Ack");
}

#[test]
fn diff_then_apply_reconstructs_exactly() {
    let old = sample_scene();
    let cases = [
        edit_field_value(&old),
        add_a_key(&old),
        remove_a_key(&old),
        change_list_length(&old),
        replace_with_different_type(),
        old.clone(),
    ];
    for new in cases {
        let patch = diff(&old, &new);
        let rebuilt = apply(&old, &patch).unwrap();
        assert_eq!(
            new, rebuilt,
            "diff+apply must reconstruct the new scene exactly"
        );
    }
}

#[test]
fn diff_of_identical_scenes_is_a_noop() {
    let scene = sample_scene();
    let patch = diff(&scene, &scene);
    let rebuilt = apply(&scene, &patch).unwrap();
    assert_eq!(scene, rebuilt);
}

#[test]
fn reordering_map_keys_reconstructs_exact_key_order() {
    let old = sample_scene();
    let new = reorder_keys(&old);
    // `Expr::Map` equality is canonical (order-insensitive), so `old == new`
    // here; the defect is STRUCTURAL -- a key reorder emitted zero ops and
    // `apply` kept the old order. Compare the order-preserving Debug form to
    // catch it.
    assert_eq!(old, new, "canonical equality ignores key order");
    assert_ne!(
        structural_repr(&old),
        structural_repr(&new),
        "the key ORDER must actually differ"
    );
    let patch = diff(&old, &new);
    let rebuilt = apply(&old, &patch).unwrap();
    assert_eq!(
        structural_repr(&rebuilt),
        structural_repr(&new),
        "apply must reconstruct the exact key order of new, not the old order"
    );
}

/// An order-preserving rendering of a value, for structural (not canonical)
/// comparison in tests.
fn structural_repr(value: &Expr) -> String {
    format!("{value:?}")
}

#[test]
fn a_scene_patch_is_itself_a_valid_scene() {
    let old = sample_scene();
    let new = edit_field_value(&old);
    let patch = diff(&old, &new);
    // The patch is a `scene/patch` node and round-trips through codec:scene.
    let mut cx = cx();
    let codec = scene_codec_symbol();
    let output = encode_with_codec(&mut cx, &codec, &patch, EncodeOptions::default()).unwrap();
    let input = match output {
        Output::Text(text) => Input::Text(text),
        Output::Bytes(bytes) => Input::Bytes(bytes),
    };
    let decoded = decode_with_codec(&mut cx, &codec, input, ReadPolicy::default()).unwrap();
    assert_eq!(patch, decoded);
    assert_eq!(new, apply(&old, &decoded).unwrap());
}

fn edit_field_value(scene: &Expr) -> Expr {
    let mut new = scene.clone();
    set_top_key(&mut new, "live", Expr::Bool(false));
    new
}

fn add_a_key(scene: &Expr) -> Expr {
    let mut new = scene.clone();
    set_top_key(&mut new, "added", Expr::String("hello".to_owned()));
    new
}

fn remove_a_key(scene: &Expr) -> Expr {
    let Expr::Map(entries) = scene else {
        unreachable!()
    };
    Expr::Map(
        entries
            .iter()
            .filter(|(key, _)| !matches!(key, Expr::Symbol(s) if &*s.name == "flags"))
            .cloned()
            .collect(),
    )
}

fn reorder_keys(scene: &Expr) -> Expr {
    let Expr::Map(entries) = scene else {
        unreachable!()
    };
    let mut reversed = entries.clone();
    reversed.reverse();
    Expr::Map(reversed)
}

fn change_list_length(scene: &Expr) -> Expr {
    let mut new = scene.clone();
    set_top_key(&mut new, "ports", Expr::Vector(vec![sym("only-one")]));
    new
}

fn replace_with_different_type() -> Expr {
    node(
        "box",
        vec![("label", Expr::String("totally different".to_owned()))],
    )
}

fn set_top_key(scene: &mut Expr, key: &str, value: Expr) {
    let Expr::Map(entries) = scene else {
        unreachable!()
    };
    if let Some(slot) = entries
        .iter_mut()
        .find(|(entry_key, _)| matches!(entry_key, Expr::Symbol(s) if &*s.name == key))
    {
        slot.1 = value;
    } else {
        entries.push((Expr::Symbol(Symbol::new(key)), value));
    }
}

fn scene_shape(name: &str) -> std::sync::Arc<dyn sim_kernel::Shape> {
    let symbol = Symbol::qualified("scene", name);
    shape_by_symbol(symbol)
}

fn scene_shape_symbol_shape() -> std::sync::Arc<dyn sim_kernel::Shape> {
    shape_by_symbol(scene_shape_symbol())
}

fn shape_by_symbol(symbol: Symbol) -> std::sync::Arc<dyn sim_kernel::Shape> {
    scene_shape_specs()
        .into_iter()
        .find(|(candidate, _)| candidate == &symbol)
        .map(|(_, shape)| shape)
        .unwrap_or_else(|| panic!("missing scene shape {symbol}"))
}
