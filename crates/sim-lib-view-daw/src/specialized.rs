//! Specialized component views for synth editors.
//!
//! These scenes are stable fixtures for component classes that need more than a
//! generic parameter panel: envelopes, algorithm routes, response plots, fixed
//! banks, step grids, polyphony activity, scopes, and SysEx comparison.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};

/// Envelope curve editor view id.
pub const ENVELOPE_CURVE_VIEW_ID: &str = "view:component-envelope-curve";
/// Algorithm routing view id.
pub const ALGORITHM_ROUTING_VIEW_ID: &str = "view:component-algorithm-routing";
/// Wiring diagram view id.
pub const WIRING_DIAGRAM_VIEW_ID: &str = "view:component-wiring-diagram";
/// Filter response view id.
pub const FILTER_RESPONSE_VIEW_ID: &str = "view:component-filter-response";
/// Resonator response view id.
pub const RESONATOR_RESPONSE_VIEW_ID: &str = "view:component-resonator-response";
/// Fixed filter bank view id.
pub const FIXED_FILTER_BANK_VIEW_ID: &str = "view:component-fixed-filter-bank";
/// Sequencer step-grid view id.
pub const SEQUENCER_STEP_GRID_VIEW_ID: &str = "view:component-sequencer-step-grid";
/// Polyphony activity map view id.
pub const POLYPHONY_ACTIVITY_VIEW_ID: &str = "view:component-polyphony-activity";
/// Scope and spectrum monitor view id.
pub const SCOPE_SPECTRUM_VIEW_ID: &str = "view:component-scope-spectrum";
/// SysEx comparison view id.
pub const SYSEX_COMPARISON_VIEW_ID: &str = "view:component-sysex-comparison";

/// Stable ids for all specialized component views.
pub const SPECIALIZED_COMPONENT_VIEW_IDS: [&str; 10] = [
    ENVELOPE_CURVE_VIEW_ID,
    ALGORITHM_ROUTING_VIEW_ID,
    WIRING_DIAGRAM_VIEW_ID,
    FILTER_RESPONSE_VIEW_ID,
    RESONATOR_RESPONSE_VIEW_ID,
    FIXED_FILTER_BANK_VIEW_ID,
    SEQUENCER_STEP_GRID_VIEW_ID,
    POLYPHONY_ACTIVITY_VIEW_ID,
    SCOPE_SPECTRUM_VIEW_ID,
    SYSEX_COMPARISON_VIEW_ID,
];

/// A component-to-view declaration used by editor routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpecializedComponentView {
    /// The stable view id.
    pub view_id: &'static str,
    /// The component id in the audio-synth component namespace.
    pub component: &'static str,
}

/// Components that declare specialized views.
pub const SPECIALIZED_DECLARING_COMPONENTS: [SpecializedComponentView; 12] = [
    SpecializedComponentView {
        view_id: ENVELOPE_CURVE_VIEW_ID,
        component: "dx7-operator-eg",
    },
    SpecializedComponentView {
        view_id: ENVELOPE_CURVE_VIEW_ID,
        component: "r700-envelope",
    },
    SpecializedComponentView {
        view_id: ALGORITHM_ROUTING_VIEW_ID,
        component: "dx7-voice",
    },
    SpecializedComponentView {
        view_id: WIRING_DIAGRAM_VIEW_ID,
        component: "dx7-voice",
    },
    SpecializedComponentView {
        view_id: FILTER_RESPONSE_VIEW_ID,
        component: "r700-vcf",
    },
    SpecializedComponentView {
        view_id: FILTER_RESPONSE_VIEW_ID,
        component: "m55-ladder-filter",
    },
    SpecializedComponentView {
        view_id: RESONATOR_RESPONSE_VIEW_ID,
        component: "ps3300-resonator",
    },
    SpecializedComponentView {
        view_id: FIXED_FILTER_BANK_VIEW_ID,
        component: "m55-fixed-filter-bank",
    },
    SpecializedComponentView {
        view_id: SEQUENCER_STEP_GRID_VIEW_ID,
        component: "r700-sequencer",
    },
    SpecializedComponentView {
        view_id: POLYPHONY_ACTIVITY_VIEW_ID,
        component: "ps3300-poly-array",
    },
    SpecializedComponentView {
        view_id: SCOPE_SPECTRUM_VIEW_ID,
        component: "scope-spectrum-monitor",
    },
    SpecializedComponentView {
        view_id: SYSEX_COMPARISON_VIEW_ID,
        component: "dx7-sysex-patch",
    },
];

/// Envelope curve fixture name.
pub const ENVELOPE_CURVE_FIXTURE: &str = "envelope-curve";
/// Algorithm routing fixture name.
pub const ALGORITHM_ROUTING_FIXTURE: &str = "algorithm-routing";
/// Filter response fixture name.
pub const FILTER_RESPONSE_FIXTURE: &str = "filter-response";
/// Resonator response fixture name.
pub const RESONATOR_RESPONSE_FIXTURE: &str = "resonator-response";
/// Fixed filter bank fixture name.
pub const FIXED_FILTER_BANK_FIXTURE: &str = "fixed-filter-bank";
/// Sequencer step-grid fixture name.
pub const SEQUENCER_STEP_GRID_FIXTURE: &str = "sequencer-step-grid";
/// Polyphony activity fixture name.
pub const POLYPHONY_ACTIVITY_FIXTURE: &str = "polyphony-activity";
/// Scope and spectrum monitor fixture name.
pub const SCOPE_SPECTRUM_FIXTURE: &str = "scope-spectrum";
/// SysEx comparison fixture name.
pub const SYSEX_COMPARISON_FIXTURE: &str = "sysex-comparison";

const SPECIALIZED_FIXTURE_NAMES: [&str; 9] = [
    ENVELOPE_CURVE_FIXTURE,
    ALGORITHM_ROUTING_FIXTURE,
    FILTER_RESPONSE_FIXTURE,
    RESONATOR_RESPONSE_FIXTURE,
    FIXED_FILTER_BANK_FIXTURE,
    SEQUENCER_STEP_GRID_FIXTURE,
    POLYPHONY_ACTIVITY_FIXTURE,
    SCOPE_SPECTRUM_FIXTURE,
    SYSEX_COMPARISON_FIXTURE,
];

/// Return every stable specialized view id.
pub fn specialized_view_ids() -> &'static [&'static str] {
    &SPECIALIZED_COMPONENT_VIEW_IDS
}

/// Return every component-to-view declaration.
pub fn specialized_declaring_components() -> &'static [SpecializedComponentView] {
    &SPECIALIZED_DECLARING_COMPONENTS
}

/// Return the fixture names covered by snapshot tests.
pub fn specialized_fixture_names() -> [&'static str; 9] {
    SPECIALIZED_FIXTURE_NAMES
}

/// Build a deterministic specialized view snapshot by fixture name.
pub fn specialized_snapshot(name: &str) -> Option<Expr> {
    match name {
        ENVELOPE_CURVE_FIXTURE => Some(envelope_curve_view()),
        ALGORITHM_ROUTING_FIXTURE => Some(algorithm_routing_view()),
        FILTER_RESPONSE_FIXTURE => Some(filter_response_view()),
        RESONATOR_RESPONSE_FIXTURE => Some(resonator_response_view()),
        FIXED_FILTER_BANK_FIXTURE => Some(fixed_filter_bank_view()),
        SEQUENCER_STEP_GRID_FIXTURE => Some(sequencer_step_grid_view()),
        POLYPHONY_ACTIVITY_FIXTURE => Some(polyphony_activity_view()),
        SCOPE_SPECTRUM_FIXTURE => Some(scope_spectrum_view()),
        SYSEX_COMPARISON_FIXTURE => Some(sysex_comparison_view()),
        _ => None,
    }
}

fn envelope_curve_view() -> Expr {
    let curve = vec![
        (0.0, 0.0),
        (0.04, 1.0),
        (0.18, 0.72),
        (0.82, 0.48),
        (1.0, 0.0),
    ];
    view_root(
        ENVELOPE_CURVE_VIEW_ID,
        "envelope-curve-editor",
        vec![
            response_plot(
                ENVELOPE_CURVE_VIEW_ID,
                "envelope-curve",
                &[("level", curve)],
            ),
            stage_table(&[
                ("attack", 0.04, 1.0),
                ("decay", 0.14, 0.72),
                ("sustain", 0.64, 0.48),
                ("release", 0.18, 0.0),
            ]),
            node(
                "button",
                vec![
                    ("role", sym("envelope-point-editor")),
                    ("action", sym("edit-envelope")),
                    ("label", Expr::String("Edit point".to_owned())),
                ],
            ),
        ],
    )
}

fn algorithm_routing_view() -> Expr {
    let routing = node(
        "matrix",
        vec![
            ("lens", id(ALGORITHM_ROUTING_VIEW_ID)),
            ("role", sym("algorithm-routing-matrix")),
            (
                "rows",
                numeric_rows(&[
                    vec![0.0, 0.0, 0.8, 0.0, 0.0, 1.0],
                    vec![0.0, 0.0, 0.0, 0.5, 1.0, 0.0],
                    vec![0.0, 0.7, 0.0, 1.0, 0.0, 0.0],
                    vec![0.6, 0.0, 0.0, 0.0, 0.0, 0.0],
                    vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                ]),
            ),
            (
                "row-labels",
                string_list(&["op1", "op2", "op3", "op4", "op5", "op6"]),
            ),
            (
                "col-labels",
                string_list(&["op1", "op2", "op3", "op4", "op5", "out"]),
            ),
            ("editable", Expr::Bool(true)),
        ],
    );
    let wiring = node(
        "graph",
        vec![
            ("lens", id(WIRING_DIAGRAM_VIEW_ID)),
            ("role", sym("wiring-diagram")),
            (
                "nodes",
                Expr::List(
                    ["op1", "op2", "op3", "op4", "op5", "op6", "out"]
                        .iter()
                        .map(|name| graph_node(name))
                        .collect(),
                ),
            ),
            (
                "edges",
                Expr::List(vec![
                    graph_edge("op6", "op1"),
                    graph_edge("op5", "op2"),
                    graph_edge("op4", "op3"),
                    graph_edge("op3", "out"),
                    graph_edge("op1", "out"),
                ]),
            ),
        ],
    );
    view_root(
        ALGORITHM_ROUTING_VIEW_ID,
        "algorithm-routing-view",
        vec![routing, wiring],
    )
}

fn filter_response_view() -> Expr {
    view_root(
        FILTER_RESPONSE_VIEW_ID,
        "filter-response-view",
        vec![
            sweep_slider("cutoff", 80.0, 12_000.0, 1_800.0),
            sweep_slider("resonance", 0.0, 1.0, 0.62),
            response_plot(
                FILTER_RESPONSE_VIEW_ID,
                "filter-response-plot",
                &[
                    (
                        "low-pass",
                        vec![
                            (20.0, -0.2),
                            (100.0, -0.1),
                            (1_000.0, -1.5),
                            (3_000.0, -8.0),
                            (10_000.0, -32.0),
                        ],
                    ),
                    (
                        "resonant",
                        vec![
                            (20.0, -0.4),
                            (100.0, -0.2),
                            (1_000.0, 2.4),
                            (3_000.0, -5.5),
                            (10_000.0, -30.0),
                        ],
                    ),
                ],
            ),
        ],
    )
}

fn resonator_response_view() -> Expr {
    view_root(
        RESONATOR_RESPONSE_VIEW_ID,
        "resonator-response-view",
        vec![
            sweep_slider("frequency", 110.0, 7_040.0, 880.0),
            sweep_slider("depth", 0.0, 1.0, 0.48),
            response_plot(
                RESONATOR_RESPONSE_VIEW_ID,
                "resonator-response-plot",
                &[
                    (
                        "bank-a",
                        vec![
                            (110.0, -12.0),
                            (220.0, -4.0),
                            (440.0, 0.0),
                            (880.0, 3.2),
                            (1_760.0, -5.0),
                        ],
                    ),
                    (
                        "bank-b",
                        vec![
                            (110.0, -16.0),
                            (220.0, -7.0),
                            (440.0, -1.0),
                            (880.0, 2.5),
                            (1_760.0, 0.2),
                        ],
                    ),
                ],
            ),
        ],
    )
}

fn fixed_filter_bank_view() -> Expr {
    view_root(
        FIXED_FILTER_BANK_VIEW_ID,
        "fixed-filter-bank-view",
        vec![
            response_plot(
                FIXED_FILTER_BANK_VIEW_ID,
                "fixed-filter-bank-plot",
                &[(
                    "band-gain",
                    vec![
                        (50.0, -3.0),
                        (120.0, 2.0),
                        (400.0, -5.0),
                        (1_200.0, 4.0),
                        (3_600.0, -1.0),
                    ],
                )],
            ),
            node(
                "table",
                vec![
                    ("role", sym("fixed-filter-bank-bands")),
                    (
                        "rows",
                        Expr::List(vec![
                            band_row("50hz", 50.0, -3.0),
                            band_row("120hz", 120.0, 2.0),
                            band_row("400hz", 400.0, -5.0),
                            band_row("1k2", 1_200.0, 4.0),
                            band_row("3k6", 3_600.0, -1.0),
                        ]),
                    ),
                ],
            ),
        ],
    )
}

fn sequencer_step_grid_view() -> Expr {
    view_root(
        SEQUENCER_STEP_GRID_VIEW_ID,
        "sequencer-step-grid",
        vec![node(
            "grid",
            vec![
                ("role", sym("sequencer-step-grid")),
                (
                    "rows",
                    numeric_rows(&[
                        vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                        vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
                        vec![0.0, 0.0, 0.7, 0.0, 0.0, 0.0, 0.4, 0.0],
                    ]),
                ),
                ("row-labels", string_list(&["gate", "accent", "cv"])),
                (
                    "col-labels",
                    string_list(&["1", "2", "3", "4", "5", "6", "7", "8"]),
                ),
                ("editable", Expr::Bool(true)),
            ],
        )],
    )
}

fn polyphony_activity_view() -> Expr {
    view_root(
        POLYPHONY_ACTIVITY_VIEW_ID,
        "polyphony-activity-view",
        vec![node(
            "grid",
            vec![
                ("role", sym("polyphony-activity-map")),
                (
                    "rows",
                    numeric_rows(&[
                        vec![1.0, 0.8, 0.4, 0.0],
                        vec![0.6, 1.0, 0.7, 0.2],
                        vec![0.2, 0.5, 1.0, 0.9],
                        vec![0.0, 0.2, 0.6, 1.0],
                    ]),
                ),
                (
                    "row-labels",
                    string_list(&["cell-1", "cell-2", "cell-3", "cell-4"]),
                ),
                ("col-labels", string_list(&["vco", "vcf", "vca", "eg"])),
            ],
        )],
    )
}

fn scope_spectrum_view() -> Expr {
    view_root(
        SCOPE_SPECTRUM_VIEW_ID,
        "scope-spectrum-monitor",
        vec![
            node(
                "waveform",
                vec![
                    ("role", sym("scope-monitor")),
                    (
                        "samples",
                        number_list(&[0.0, 0.32, 0.74, 0.96, 0.62, 0.0, -0.58, -0.92, -0.31]),
                    ),
                ],
            ),
            node(
                "spectrum",
                vec![
                    ("role", sym("spectrum-monitor")),
                    (
                        "bins",
                        number_list(&[0.0, -5.0, -12.0, -18.0, -26.0, -40.0]),
                    ),
                ],
            ),
        ],
    )
}

fn sysex_comparison_view() -> Expr {
    view_root(
        SYSEX_COMPARISON_VIEW_ID,
        "sysex-comparison-view",
        vec![
            node(
                "table",
                vec![
                    ("role", sym("sysex-format-comparison")),
                    (
                        "rows",
                        Expr::List(vec![
                            sysex_row("hex", "f0 43 00 09 20 00 7f f7"),
                            sysex_row("binary", "11110000 01000011 00000000 00001001"),
                            sysex_row("lisp", "(dx7-patch :algorithm 5 :feedback 7)"),
                        ]),
                    ),
                ],
            ),
            node(
                "badge",
                vec![
                    ("role", sym("round-trip-probe")),
                    ("status", sym("ok")),
                    (
                        "label",
                        Expr::String("lossless codec:lisp -> codec:binary".to_owned()),
                    ),
                ],
            ),
        ],
    )
}

fn view_root(lens: &str, role: &str, children: Vec<Expr>) -> Expr {
    node(
        "stack",
        vec![
            ("lens", id(lens)),
            ("role", sym(role)),
            ("dir", sym("column")),
            ("declaring-components", declaring_components(lens)),
            ("children", Expr::List(children)),
        ],
    )
}

fn declaring_components(view_id: &str) -> Expr {
    Expr::List(
        SPECIALIZED_DECLARING_COMPONENTS
            .iter()
            .filter(|entry| entry.view_id == view_id)
            .map(|entry| qsym("audio-synth/component", entry.component))
            .collect(),
    )
}

fn stage_table(stages: &[(&str, f64, f64)]) -> Expr {
    node(
        "table",
        vec![
            ("role", sym("envelope-stage-table")),
            (
                "rows",
                Expr::List(
                    stages
                        .iter()
                        .map(|(stage, time, level)| {
                            data_map(vec![
                                ("stage", sym(stage)),
                                ("time", number(*time)),
                                ("level", number(*level)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ],
    )
}

fn response_plot(lens: &str, role: &str, series: &[(&str, Vec<(f64, f64)>)]) -> Expr {
    let (min_x, max_x, min_y, max_y) = bounds(series);
    node(
        "plot",
        vec![
            ("lens", id(lens)),
            ("role", sym(role)),
            (
                "axes",
                data_map(vec![("x", axis(min_x, max_x)), ("y", axis(min_y, max_y))]),
            ),
            (
                "series",
                Expr::List(
                    series
                        .iter()
                        .map(|(name, points)| {
                            data_map(vec![
                                ("name", sym(name)),
                                ("style", sym("line")),
                                (
                                    "points",
                                    Expr::List(
                                        points
                                            .iter()
                                            .map(|(x, y)| {
                                                data_map(vec![("x", number(*x)), ("y", number(*y))])
                                            })
                                            .collect(),
                                    ),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
        ],
    )
}

fn bounds(series: &[(&str, Vec<(f64, f64)>)]) -> (f64, f64, f64, f64) {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for (_, points) in series {
        for (x, y) in points {
            bounds = Some(match bounds {
                Some((min_x, max_x, min_y, max_y)) => {
                    (min_x.min(*x), max_x.max(*x), min_y.min(*y), max_y.max(*y))
                }
                None => (*x, *x, *y, *y),
            });
        }
    }
    bounds.unwrap_or((0.0, 1.0, 0.0, 1.0))
}

fn axis(min: f64, max: f64) -> Expr {
    data_map(vec![("min", number(min)), ("max", number(max))])
}

fn sweep_slider(param: &str, min: f64, max: f64, value: f64) -> Expr {
    node(
        "slider",
        vec![
            ("role", sym("response-sweep")),
            ("param", sym(param)),
            ("min", number(min)),
            ("max", number(max)),
            ("value", number(value)),
        ],
    )
}

fn band_row(name: &str, frequency: f64, gain: f64) -> Expr {
    data_map(vec![
        ("name", Expr::String(name.to_owned())),
        ("frequency", number(frequency)),
        ("gain", number(gain)),
    ])
}

fn sysex_row(format: &str, value: &str) -> Expr {
    data_map(vec![
        ("format", sym(format)),
        ("value", Expr::String(value.to_owned())),
    ])
}

fn graph_node(name: &str) -> Expr {
    node(
        "node",
        vec![("id", sym(name)), ("label", Expr::String(name.to_owned()))],
    )
}

fn graph_edge(from: &str, to: &str) -> Expr {
    node("edge", vec![("from", sym(from)), ("to", sym(to))])
}

fn numeric_rows(rows: &[Vec<f64>]) -> Expr {
    Expr::List(
        rows.iter()
            .map(|row| Expr::List(row.iter().map(|value| number(*value)).collect()))
            .collect(),
    )
}

fn number_list(values: &[f64]) -> Expr {
    Expr::List(values.iter().map(|value| number(*value)).collect())
}

fn string_list(values: &[&str]) -> Expr {
    Expr::List(
        values
            .iter()
            .map(|value| Expr::String((*value).to_owned()))
            .collect(),
    )
}

fn id(name: &str) -> Expr {
    Expr::Symbol(Symbol::new(name))
}

fn qsym(namespace: &str, name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified(namespace, name))
}

use sim_value::build::float as number;
