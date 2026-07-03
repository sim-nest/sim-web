//! The multi-codec lens and round-trip probe.
//!
//! One value, several codecs, side by side. Each codec's rendering is a
//! `scene/embed` carrying the encoded form and a round-trip badge. The probe
//! encodes then decodes and reports whether the value survived and, if not,
//! that there was loss. Read-construct and codec output position are surfaced
//! where relevant; broad read-eval is never exposed here (it stays
//! capability-gated elsewhere).

use sim_codec::{
    CodecPrism, Input, Output, PrismDiagnostic, PrismOutput, RuntimeCodecPrism, decode_with_codec,
    encode_with_codec,
};
use sim_kernel::{Cx, EncodeOptions, EncodePosition, Expr, ReadPolicy, Symbol};
use sim_lib_scene::{data_map, node, sym};

/// The multi-codec lens id.
pub const MULTI_CODEC_LENS: &str = "view:codec-multi";

/// The SysEx comparison lens id.
pub const SYSEX_COMPARISON_LENS: &str = "view:codec-sysex-comparison";

/// The outcome of a round-trip probe through one codec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeResult {
    /// The encoded form, rendered for display.
    pub encoded: String,
    /// Whether decoding reproduced the original value.
    pub lossless: bool,
    /// Whether the Prism parse/encode/reparse proof preserved semantic id.
    pub semantic_identity: bool,
    /// Stable semantic id from the Prism parse pass.
    pub semantic_id: Option<String>,
    /// Number of source spans discovered by the Prism parse pass.
    pub span_count: usize,
    /// Loss and parse diagnostics collected by the Prism.
    pub diagnostics: Vec<String>,
    /// Whether the inspected input is treated as trusted executable input.
    pub trusted_executable: bool,
    /// Suggested alternate codec surfaces for comparison.
    pub alternates: Vec<Symbol>,
}

impl ProbeResult {
    /// Builds a passing probe result for callers that already have a proof.
    pub fn lossless(encoded: impl Into<String>) -> Self {
        Self {
            encoded: encoded.into(),
            lossless: true,
            semantic_identity: true,
            semantic_id: None,
            span_count: 0,
            diagnostics: Vec::new(),
            trusted_executable: false,
            alternates: Vec::new(),
        }
    }
}

/// Probe a value's round-trip through one codec.
pub fn roundtrip_probe(cx: &mut Cx, codec: &Symbol, value: &Expr) -> ProbeResult {
    match encode_with_codec(cx, codec, value, EncodeOptions::default()) {
        Ok(output) => {
            let (encoded, input, prism_output) = render_output(output);
            let decoded_lossless = decode_with_codec(cx, codec, input, ReadPolicy::default())
                .map(|decoded| decoded.canonical_eq(value))
                .unwrap_or(false);
            let prism = prism_for_codec(codec);
            let proof = match prism_output {
                PrismOutput::Text(text) => prism.round_trip(cx, &text, EncodePosition::Data),
                PrismOutput::Bytes(bytes) => {
                    prism.round_trip_bytes(cx, &bytes, EncodePosition::Data)
                }
            };
            let semantic_identity = proof.loss_report.semantic_identity;
            let diagnostics = diagnostic_text(&proof.loss_report.diagnostics);
            ProbeResult {
                encoded,
                lossless: decoded_lossless && proof.loss_report.lossless,
                semantic_identity,
                semantic_id: proof.parse.semantic_id.as_ref().map(|id| id.stable.clone()),
                span_count: proof.parse.span_map.len(),
                diagnostics,
                trusted_executable: proof.parse.inspection.trusted_executable,
                alternates: suggested_alternates(codec),
            }
        }
        Err(error) => ProbeResult {
            encoded: format!("<encode error: {error}>"),
            lossless: false,
            semantic_identity: false,
            semantic_id: None,
            span_count: 0,
            diagnostics: vec![format!("encode-error: {error}")],
            trusted_executable: false,
            alternates: suggested_alternates(codec),
        },
    }
}

fn render_output(output: Output) -> (String, Input, PrismOutput) {
    match output {
        Output::Text(text) => (
            text.clone(),
            Input::Text(text.clone()),
            PrismOutput::Text(text),
        ),
        Output::Bytes(bytes) => {
            let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
            let display = format!("{} bytes: {hex}", bytes.len());
            (
                display,
                Input::Bytes(bytes.clone()),
                PrismOutput::Bytes(bytes),
            )
        }
    }
}

fn prism_for_codec(codec: &Symbol) -> RuntimeCodecPrism {
    match (&codec.namespace, &*codec.name) {
        (Some(namespace), "chat") if &**namespace == "codec" => {
            RuntimeCodecPrism::domain(codec.clone(), "chat transcript")
        }
        (Some(namespace), "mcp") if &**namespace == "codec" => {
            RuntimeCodecPrism::domain(codec.clone(), "MCP envelope")
        }
        (Some(namespace), "binary") if &**namespace == "codec" => {
            RuntimeCodecPrism::binary(codec.clone())
        }
        (Some(namespace), "binary-base64") if &**namespace == "codec" => {
            RuntimeCodecPrism::binary_base64(codec.clone())
        }
        _ => RuntimeCodecPrism::general(codec.clone()),
    }
}

fn suggested_alternates(codec: &Symbol) -> Vec<Symbol> {
    let installed = ["lisp", "algol", "json", "binary", "binary-base64"];
    installed
        .iter()
        .map(|name| Symbol::qualified("codec", *name))
        .filter(|candidate| candidate != codec)
        .collect()
}

fn diagnostic_text(diagnostics: &[PrismDiagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect()
}

/// Open one value through several codecs at once, each as a `scene/embed` with a
/// round-trip badge.
pub fn multi_codec_view(cx: &mut Cx, codecs: &[Symbol], value: &Expr) -> Expr {
    let panels = codecs
        .iter()
        .map(|codec| codec_panel(cx, codec, value))
        .collect();
    node(
        "stack",
        vec![
            ("role", sym("multi-codec")),
            ("dir", sym("row")),
            ("children", Expr::List(panels)),
        ],
    )
}

/// Compare the same SysEx payload as hex, binary, and Lisp data, with a
/// round-trip probe result.
pub fn sysex_comparison_view(hex: &str, bytes: &[u8], lisp: &str, probe: &ProbeResult) -> Expr {
    let status = if probe.lossless { "ok" } else { "warn" };
    node(
        "stack",
        vec![
            ("lens", Expr::Symbol(Symbol::new(SYSEX_COMPARISON_LENS))),
            ("role", sym("sysex-comparison-view")),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![
                    node(
                        "table",
                        vec![
                            ("role", sym("sysex-format-comparison")),
                            (
                                "rows",
                                Expr::List(vec![
                                    comparison_row("hex", hex),
                                    comparison_row("binary", &binary_display(bytes)),
                                    comparison_row("lisp", lisp),
                                ]),
                            ),
                        ],
                    ),
                    node(
                        "badge",
                        vec![
                            ("role", sym("round-trip-probe")),
                            ("status", sym(status)),
                            ("label", Expr::String(probe.encoded.clone())),
                        ],
                    ),
                ]),
            ),
        ],
    )
}

fn codec_panel(cx: &mut Cx, codec: &Symbol, value: &Expr) -> Expr {
    let probe = roundtrip_probe(cx, codec, value);
    let prism_report = prism_report(&probe);
    let badge = node(
        "badge",
        vec![
            ("status", sym(if probe.lossless { "ok" } else { "warn" })),
            (
                "label",
                Expr::String(
                    if probe.lossless {
                        "round-trips"
                    } else {
                        "loss"
                    }
                    .to_owned(),
                ),
            ),
        ],
    );
    let inner = node(
        "box",
        vec![
            ("role", sym("codec-prism")),
            (
                "prism",
                data_map(vec![
                    (
                        "semantic-id",
                        Expr::String(probe.semantic_id.clone().unwrap_or_default()),
                    ),
                    ("semantic-identity", bool_expr(probe.semantic_identity)),
                    ("span-count", int_expr(probe.span_count)),
                    ("trusted-executable", bool_expr(probe.trusted_executable)),
                    (
                        "alternates",
                        Expr::List(probe.alternates.iter().cloned().map(Expr::Symbol).collect()),
                    ),
                ]),
            ),
            (
                "children",
                Expr::List(vec![
                    node("text", vec![("text", Expr::String(codec.to_string()))]),
                    badge,
                    prism_report,
                    node(
                        "field",
                        vec![
                            ("datatype", sym("text")),
                            ("value", Expr::String(probe.encoded)),
                        ],
                    ),
                ]),
            ),
        ],
    );
    node(
        "embed",
        vec![("lens", Expr::Symbol(codec.clone())), ("scene", inner)],
    )
}

fn prism_report(probe: &ProbeResult) -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("prism-diagnostics")),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![
                    node(
                        "badge",
                        vec![
                            ("role", sym("prism-loss-report")),
                            ("status", sym(if probe.lossless { "ok" } else { "warn" })),
                            (
                                "label",
                                Expr::String(
                                    if probe.semantic_identity {
                                        "semantic identity"
                                    } else {
                                        "semantic loss"
                                    }
                                    .to_owned(),
                                ),
                            ),
                        ],
                    ),
                    data_map(vec![
                        ("spans", int_expr(probe.span_count)),
                        (
                            "diagnostics",
                            Expr::List(
                                probe
                                    .diagnostics
                                    .iter()
                                    .cloned()
                                    .map(Expr::String)
                                    .collect(),
                            ),
                        ),
                    ]),
                ]),
            ),
        ],
    )
}

fn bool_expr(value: bool) -> Expr {
    Expr::Symbol(Symbol::new(if value { "true" } else { "false" }))
}

fn int_expr(value: usize) -> Expr {
    Expr::String(value.to_string())
}

fn comparison_row(format: &str, value: &str) -> Expr {
    data_map(vec![
        ("format", Expr::Symbol(Symbol::new(format))),
        ("value", Expr::String(value.to_owned())),
    ])
}

fn binary_display(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:08b}"))
        .collect::<Vec<_>>()
        .join(" ")
}
