//! Tests for the multi-codec and Shape lenses.

use std::sync::Arc;

use sim_codec::{CodecPrism, Output, RuntimeCodecPrism, encode_with_codec};
use sim_kernel::{Cx, EncodeOptions, Expr, NumberLiteral, Symbol, testing::eager_cx};
use sim_shape::{ExprKind, ExprKindShape, shape_value};

use crate::multicodec::{
    ProbeResult, SYSEX_COMPARISON_LENS, multi_codec_view, roundtrip_probe, sysex_comparison_view,
};
use crate::shape::shape_view;

fn cx() -> Cx {
    let mut cx = eager_cx();
    sim_test_support::register_core_classes(&mut cx);
    sim_test_support::register_f64_number_domain(&mut cx);
    let lisp = sim_codec_lisp::LispCodecLib::new(cx.registry_mut().fresh_codec_id()).unwrap();
    cx.load_lib(&lisp).unwrap();
    let json = sim_codec_json::JsonCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&json).unwrap();
    let binary = sim_codec_binary::BinaryCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&binary).unwrap();
    let binary_base64 =
        sim_codec_binary_base64::BinaryBase64CodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&binary_base64).unwrap();
    let algol = sim_codec_algol::AlgolCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&algol).unwrap();
    let chat = sim_codec_chat::ChatCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&chat).unwrap();
    let mcp = sim_codec_mcp::McpCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&mcp).unwrap();
    cx
}

fn codecs() -> Vec<Symbol> {
    ["lisp", "json", "binary", "binary-base64", "algol"]
        .iter()
        .map(|name| Symbol::qualified("codec", *name))
        .collect()
}

fn sample_value() -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("a")),
            Expr::Number(NumberLiteral {
                domain: Symbol::qualified("numbers", "f64"),
                canonical: "1".to_owned(),
            }),
        ),
        (
            Expr::Symbol(Symbol::new("b")),
            Expr::String("two".to_owned()),
        ),
    ])
}

fn chat_value() -> Expr {
    sim_codec_chat::model_response_expr(
        Symbol::new("local-runner"),
        "fixture-model",
        vec![sim_codec_chat::text_part("ready")],
        Symbol::new("stop"),
    )
}

fn mcp_value() -> Expr {
    sim_codec_mcp::envelope_to_expr(&sim_codec_mcp::McpEnvelope::Request(
        sim_codec_mcp::McpRequest {
            id: Expr::String("round-trip".to_owned()),
            method: "tools/list".to_owned(),
            params: Expr::Map(Vec::new()),
        },
    ))
}

#[test]
fn one_value_opens_through_several_codecs_with_roundtrip_status() {
    let mut cx = cx();
    let value = sample_value();
    let scene = multi_codec_view(&mut cx, &codecs(), &value);
    sim_lib_scene::validate_scene(&scene).expect("the multi-codec scene is valid");

    // The lisp codec round-trips this value losslessly.
    let probe = roundtrip_probe(&mut cx, &Symbol::qualified("codec", "lisp"), &value);
    assert!(probe.lossless, "lisp round-trip should be lossless");
    assert!(
        probe.semantic_identity,
        "lisp Prism proof should preserve semantic identity"
    );
    assert!(probe.semantic_id.is_some());
    assert!(probe.span_count > 0);
    assert!(!probe.encoded.is_empty());
    assert!(contains_role(&scene, "codec-prism"));
    assert!(contains_role(&scene, "prism-diagnostics"));
    assert!(contains_role(&scene, "prism-loss-report"));
}

#[test]
fn sysex_payload_opens_as_hex_binary_lisp_and_probe() {
    let probe = ProbeResult::lossless("round-trips");
    let scene = sysex_comparison_view(
        "f0 43 00 09 20 00 7f f7",
        &[0xf0, 0x43, 0x00, 0x09],
        "(dx7-patch :algorithm 5 :feedback 7)",
        &probe,
    );
    sim_lib_scene::validate_scene(&scene).expect("the SysEx comparison scene is valid");
    assert_eq!(
        field(&scene, "lens"),
        Some(Expr::Symbol(Symbol::new(SYSEX_COMPARISON_LENS)))
    );
    assert_eq!(
        field(&scene, "role"),
        Some(Expr::Symbol(Symbol::new("sysex-comparison-view")))
    );
    assert!(contains_role(&scene, "sysex-format-comparison"));
    assert!(contains_role(&scene, "round-trip-probe"));
}

#[test]
fn codec_prism_matrix_roundtrips_installed_surfaces() {
    let mut cx = cx();
    let general_value = sample_value();
    for name in ["lisp", "json", "algol"] {
        let probe = roundtrip_probe(&mut cx, &Symbol::qualified("codec", name), &general_value);
        assert!(probe.lossless, "{name} should round-trip: {probe:?}");
        assert!(
            probe.semantic_identity,
            "{name} Prism proof should preserve semantic identity"
        );
        assert!(probe.span_count > 0, "{name} should surface spans");
    }

    for name in ["binary", "binary-base64"] {
        let probe = roundtrip_probe(&mut cx, &Symbol::qualified("codec", name), &general_value);
        assert!(probe.lossless, "{name} should round-trip: {probe:?}");
        assert!(
            !probe.trusted_executable,
            "{name} inspection must not trust executable input"
        );
    }

    for (name, value) in [("chat", chat_value()), ("mcp", mcp_value())] {
        let probe = roundtrip_probe(&mut cx, &Symbol::qualified("codec", name), &value);
        assert!(probe.lossless, "{name} should round-trip: {probe:?}");
        assert!(
            probe.semantic_identity,
            "{name} Prism proof should preserve domain semantic identity"
        );
    }
}

#[test]
fn domain_prisms_fail_closed_outside_their_domain() {
    let mut cx = cx();
    let chat = RuntimeCodecPrism::domain(Symbol::qualified("codec", "chat"), "chat transcript");
    let mcp = RuntimeCodecPrism::domain(Symbol::qualified("codec", "mcp"), "MCP envelope");

    let chat_result = chat.parse(&mut cx, "not a chat transcript");
    assert!(
        chat_result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "domain-rejected")
    );
    assert!(chat_result.semantic_id.is_none());

    let mcp_result = mcp.parse(&mut cx, "{\"jsonrpc\":\"2.0\",\"method\":5}");
    assert!(
        mcp_result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "domain-rejected")
    );
    assert!(mcp_result.semantic_id.is_none());
}

#[test]
fn binary_prisms_inspect_untrusted_bytes() {
    let mut cx = cx();
    let codec = Symbol::qualified("codec", "binary");
    let output =
        encode_with_codec(&mut cx, &codec, &sample_value(), EncodeOptions::default()).unwrap();
    let Output::Bytes(bytes) = output else {
        panic!("binary codec should produce bytes");
    };

    let binary = RuntimeCodecPrism::binary(codec);
    let parsed = binary.parse_bytes(&mut cx, &bytes);
    assert!(parsed.semantic_id.is_some());
    assert_eq!(parsed.inspection.input, sim_codec::PrismInputKind::Bytes);
    assert!(!parsed.inspection.trusted_executable);

    let base64_probe = roundtrip_probe(
        &mut cx,
        &Symbol::qualified("codec", "binary-base64"),
        &sample_value(),
    );
    assert!(base64_probe.lossless, "{base64_probe:?}");
    assert!(!base64_probe.trusted_executable);
}

#[test]
fn the_shape_lens_shows_a_match_and_a_counterexample() {
    let mut cx = cx();
    let shape = shape_value(
        Symbol::qualified("test", "Number"),
        Arc::new(ExprKindShape::new(ExprKind::Number)),
    );
    let matching = Expr::Number(NumberLiteral {
        domain: Symbol::new("i64"),
        canonical: "7".to_owned(),
    });
    let counterexample = Expr::String("not a number".to_owned());

    let scene = shape_view(&mut cx, &shape, &matching, &counterexample).unwrap();
    sim_lib_scene::validate_scene(&scene).expect("the shape scene is valid");

    // The matching value is accepted; the counterexample is rejected.
    let shape_obj = shape.object().as_shape().unwrap();
    assert!(shape_obj.check_expr(&mut cx, &matching).unwrap().accepted);
    let counter = shape_obj.check_expr(&mut cx, &counterexample).unwrap();
    assert!(!counter.accepted, "the counterexample must fail to match");
    assert!(
        !counter.diagnostics.is_empty(),
        "a failing match carries a diagnostic explaining the rejection"
    );
}

#[test]
fn a_value_and_its_shape_are_inspectable_in_one_workspace() {
    // The first-demo requirement: one value through multiple codecs and its
    // Shape, with round-trip status and a counterexample, side by side.
    let mut cx = cx();
    let value = Expr::Number(NumberLiteral {
        domain: Symbol::new("i64"),
        canonical: "42".to_owned(),
    });
    let codec_panel = multi_codec_view(&mut cx, &codecs(), &value);
    let shape = shape_value(
        Symbol::qualified("test", "Number"),
        Arc::new(ExprKindShape::new(ExprKind::Number)),
    );
    let shape_panel = shape_view(&mut cx, &shape, &value, &Expr::String("x".to_owned())).unwrap();

    let workspace = sim_lib_scene::node(
        "stack",
        vec![("children", Expr::List(vec![codec_panel, shape_panel]))],
    );
    sim_lib_scene::validate_scene(&workspace).expect("the combined workspace is a valid scene");
}

fn field(map: &Expr, name: &str) -> Option<Expr> {
    let Expr::Map(entries) = map else { return None };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(s) if &*s.name == name).then(|| value.clone())
    })
}

fn contains_role(expr: &Expr, role: &str) -> bool {
    field(expr, "role") == Some(Expr::Symbol(Symbol::new(role)))
        || expr_children(expr)
            .iter()
            .any(|child| contains_role(child, role))
}

fn expr_children(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) | Expr::Block(items) => {
            items.iter().collect()
        }
        Expr::Map(entries) => entries
            .iter()
            .flat_map(|(key, value)| [key, value])
            .collect(),
        Expr::Call { operator, args } => std::iter::once(operator.as_ref()).chain(args).collect(),
        Expr::Infix { left, right, .. } => vec![left, right],
        Expr::Prefix { arg, .. } | Expr::Postfix { arg, .. } => vec![arg],
        Expr::Quote { expr, .. } => vec![expr],
        Expr::Annotated { expr, annotations } => std::iter::once(expr.as_ref())
            .chain(annotations.iter().map(|(_, value)| value))
            .collect(),
        Expr::Extension { payload, .. } => vec![payload],
        _ => Vec::new(),
    }
}
