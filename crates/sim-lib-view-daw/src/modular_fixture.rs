use sim_kernel::{Expr, Symbol};
use sim_value::build::{int, list, map, qsym, sym, text, vector};

use crate::modular::{
    BuilderValidation, COMPONENT_BUILDER_PATCH_FORMAT, COMPONENT_BUILDER_VALIDATION_CODES,
};

pub(crate) fn fixture_palette() -> Expr {
    map(vec![
        ("tag", qsym("audio-synth", "component-palette")),
        ("patch-format", text(COMPONENT_BUILDER_PATCH_FORMAT)),
        (
            "category-filter",
            qsym("audio-synth/component-category", "exact"),
        ),
        (
            "capability-filter",
            qsym("audio-synth/component-capability", "editable"),
        ),
        (
            "items",
            vector(vec![
                palette_item(
                    "r700-vco",
                    "R700 VCO",
                    "exact",
                    &["realtime-safe", "editable"],
                ),
                palette_item(
                    "dx7",
                    "DX7",
                    "exact",
                    &["editable", "traceable", "specialized-view"],
                ),
                palette_item("adapter", "Adapter", "compatible", &["traceable"]),
            ]),
        ),
    ])
}

pub(crate) fn fixture_patch() -> Expr {
    map(vec![
        ("tag", qsym("audio-synth", "patch")),
        ("name", qsym("audio-synth/patch", "builder")),
        (
            "modules",
            list(vec![
                module(
                    "keyboard",
                    "keyboard",
                    Vec::new(),
                    vec![jack("gate"), jack("pitch")],
                ),
                module("env", "envelope", vec![jack("gate")], vec![jack("cv")]),
                module(
                    "vco",
                    "oscillator",
                    vec![jack("pitch")],
                    vec![jack("audio")],
                ),
                module(
                    "vca",
                    "amplifier",
                    vec![jack("audio"), jack("cv")],
                    vec![jack("audio")],
                ),
            ]),
        ),
        (
            "cords",
            list(vec![
                cord("keyboard", "gate", "env", "gate"),
                cord("keyboard", "pitch", "vco", "pitch"),
                cord("vco", "audio", "vca", "audio"),
                cord("env", "cv", "vca", "cv"),
            ]),
        ),
    ])
}

pub(crate) fn fixture_sections() -> Expr {
    vector(vec![
        section("mono-lead", "Mono Lead", true, 1),
        section("poly-pad", "Poly Pad", true, 8),
        section("bass", "Bass", false, 1),
    ])
}

pub(crate) fn invalid_patch_validations() -> Vec<BuilderValidation> {
    COMPONENT_BUILDER_VALIDATION_CODES
        .iter()
        .map(|code| {
            BuilderValidation::new(code, Some(Symbol::new("vca")), validation_message(code))
        })
        .collect()
}

fn palette_item(id: &str, label: &str, category: &str, capabilities: &[&str]) -> Expr {
    map(vec![
        ("id", qsym("audio-synth/component", id)),
        ("label", text(label)),
        ("category", qsym("audio-synth/component-category", category)),
        (
            "capabilities",
            vector(
                capabilities
                    .iter()
                    .map(|capability| qsym("audio-synth/component-capability", capability))
                    .collect(),
            ),
        ),
        ("implemented", Expr::Bool(true)),
    ])
}

fn module(id: &str, kind: &str, inputs: Vec<Expr>, outputs: Vec<Expr>) -> Expr {
    map(vec![
        ("id", sym(id)),
        ("kind", qsym("audio-synth/module", kind)),
        ("inputs", list(inputs)),
        ("outputs", list(outputs)),
    ])
}

fn jack(name: &str) -> Expr {
    map(vec![("name", sym(name))])
}

fn cord(from_module: &str, from_jack: &str, to_module: &str, to_jack: &str) -> Expr {
    map(vec![
        (
            "from",
            map(vec![("module", sym(from_module)), ("jack", sym(from_jack))]),
        ),
        (
            "to",
            map(vec![("module", sym(to_module)), ("jack", sym(to_jack))]),
        ),
    ])
}

fn section(id: &str, label: &str, enabled: bool, voices: i64) -> Expr {
    map(vec![
        ("id", sym(id)),
        ("label", text(label)),
        ("enabled", Expr::Bool(enabled)),
        ("voices", int(voices)),
        ("clock", qsym("clock", "sample")),
        ("rate", sym("audio-rate")),
    ])
}

fn validation_message(code: &str) -> &'static str {
    match code {
        "missing-input" => "required input is not patched",
        "cycle" => "cycle requires an explicit delay",
        "feedback-delay" => "feedback delay is not declared",
        "clock-mismatch" => "clock domains do not match",
        "rate-mismatch" => "signal rates do not match",
        "gate-s-trigger-mismatch" => "gate/S-trigger convention mismatch",
        _ => "builder validation failed",
    }
}
