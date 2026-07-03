//! DX7 instrument editor scenes.

use sim_kernel::Expr;
use sim_lib_music_synth::{
    DX7_EDITOR_FIXTURE_NAMES, DX7_EDITOR_ROUTE_NAME, DX7_EDITOR_VIEW_ID,
    INSTRUMENT_EDITOR_DESCRIPTORS, InstrumentEditorDescriptor,
};
use sim_lib_scene::{node, sym};

use crate::instrument::{
    action, editor_root, graph, number, number_list, row, slider, table, validation_panel,
};

/// DX7 editor route name.
pub const DX7_EDITOR_ROUTE: &str = DX7_EDITOR_ROUTE_NAME;
/// DX7 editor view id.
pub const DX7_EDITOR_VIEW: &str = DX7_EDITOR_VIEW_ID;

/// Return DX7 editor fixture names.
pub fn dx7_editor_fixture_names() -> &'static [&'static str] {
    &DX7_EDITOR_FIXTURE_NAMES
}

/// Build a deterministic DX7 editor snapshot.
pub fn dx7_editor_snapshot(name: &str) -> Option<Expr> {
    DX7_EDITOR_FIXTURE_NAMES
        .contains(&name)
        .then(|| dx7_editor_view(name))
}

/// Build the DX7 editor view for a fixture name.
pub fn dx7_editor_view(fixture_name: &str) -> Expr {
    let descriptor = descriptor();
    let invalid = fixture_name == DX7_EDITOR_FIXTURE_NAMES[2];
    let empty = fixture_name == DX7_EDITOR_FIXTURE_NAMES[1];
    let all_algorithms = fixture_name == DX7_EDITOR_FIXTURE_NAMES[3];
    let mut children = vec![
        action("sysex-import", "import-sysex", "Import SysEx"),
        algorithm_panel(all_algorithms),
        operator_grid(empty),
        eg_panel(empty),
        pitch_panel(),
        lfo_panel(),
        compare_view(all_algorithms),
        trace_view(),
    ];
    if invalid {
        children.push(validation_panel("invalid-sysex-checksum", "sysex"));
        children.push(validation_panel("operator-output-range", "operator-4"));
    }
    editor_root(descriptor, "dx7-editor", fixture_name, children)
}

fn descriptor() -> &'static InstrumentEditorDescriptor {
    INSTRUMENT_EDITOR_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.instrument == "dx7")
        .expect("DX7 editor descriptor")
}

fn algorithm_panel(all_algorithms: bool) -> Expr {
    let rows = if all_algorithms {
        (1..=32)
            .map(|id| {
                row(vec![
                    ("algorithm", number(id as f64)),
                    ("operators", number(6.0)),
                    ("carriers", number((id % 6 + 1) as f64)),
                ])
            })
            .collect()
    } else {
        vec![row(vec![
            ("algorithm", number(5.0)),
            ("operators", number(6.0)),
            ("carriers", number(3.0)),
        ])]
    };
    node(
        "stack",
        vec![
            ("role", sym("algorithm-editor")),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![
                    graph(
                        "algorithm-routing",
                        &["op1", "op2", "op3", "op4", "op5", "op6", "out"],
                        &[
                            ("op6", "op1"),
                            ("op5", "op2"),
                            ("op4", "op3"),
                            ("op3", "out"),
                            ("op1", "out"),
                        ],
                    ),
                    table(
                        if all_algorithms {
                            "all-algorithm-compare"
                        } else {
                            "algorithm-summary"
                        },
                        rows,
                    ),
                ]),
            ),
        ],
    )
}

fn operator_grid(empty: bool) -> Expr {
    let rows = if empty {
        Vec::new()
    } else {
        (1..=6)
            .map(|index| {
                row(vec![
                    ("operator", number(index as f64)),
                    ("ratio", number(0.5 + index as f64 * 0.25)),
                    ("output-level", number(64.0 + index as f64 * 3.0)),
                    ("detune", number(index as f64 - 3.0)),
                ])
            })
            .collect()
    };
    table("operator-grid", rows)
}

fn eg_panel(empty: bool) -> Expr {
    let rows = if empty {
        Vec::new()
    } else {
        (1..=6)
            .map(|index| {
                row(vec![
                    ("operator", number(index as f64)),
                    ("rate-1", number(80.0 - index as f64)),
                    ("level-1", number(99.0)),
                    ("rate-4", number(30.0 + index as f64)),
                    ("level-4", number(0.0)),
                ])
            })
            .collect()
    };
    table("eg-editor", rows)
}

fn pitch_panel() -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("pitch-editor")),
            ("dir", sym("row")),
            (
                "children",
                Expr::List(vec![
                    slider("pitch-envelope-rate", "pitch-rate", 0.0, 99.0, 52.0),
                    slider("pitch-bend-range", "pitch-bend", 0.0, 12.0, 2.0),
                ]),
            ),
        ],
    )
}

fn lfo_panel() -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("lfo-editor")),
            ("dir", sym("row")),
            (
                "children",
                Expr::List(vec![
                    slider("lfo-speed", "lfo-speed", 0.0, 99.0, 35.0),
                    slider("lfo-delay", "lfo-delay", 0.0, 99.0, 12.0),
                    slider("lfo-pitch-depth", "pitch-mod-depth", 0.0, 99.0, 18.0),
                ]),
            ),
        ],
    )
}

fn compare_view(all_algorithms: bool) -> Expr {
    let rows = if all_algorithms {
        (1..=32)
            .map(|id| {
                row(vec![
                    ("patch", Expr::String(format!("dx7-algorithm-{id:02}"))),
                    ("algorithm", number(id as f64)),
                    ("trace-peaked", Expr::Bool(id % 3 == 0)),
                ])
            })
            .collect()
    } else {
        vec![
            row(vec![
                ("patch", Expr::String("edit-buffer".to_owned())),
                ("algorithm", number(5.0)),
                ("trace-peaked", Expr::Bool(false)),
            ]),
            row(vec![
                ("patch", Expr::String("stored".to_owned())),
                ("algorithm", number(5.0)),
                ("trace-peaked", Expr::Bool(false)),
            ]),
        ]
    };
    table("dx7-compare-view", rows)
}

fn trace_view() -> Expr {
    node(
        "waveform",
        vec![
            ("role", sym("dx7-trace-view")),
            (
                "samples",
                number_list(&[0.0, 0.18, 0.63, 0.9, 0.42, -0.1, -0.4, 0.0]),
            ),
        ],
    )
}
