//! Korg PS-3300 instrument editor scenes.

use sim_kernel::Expr;
use sim_lib_music_synth::{
    INSTRUMENT_EDITOR_DESCRIPTORS, InstrumentEditorDescriptor, PS3300_EDITOR_FIXTURE_NAMES,
    PS3300_EDITOR_ROUTE_NAME, PS3300_EDITOR_VIEW_ID,
};
use sim_lib_scene::{node, sym};

use crate::instrument::{
    action, editor_root, graph, grid, number, row, slider, table, validation_panel,
};

/// Korg PS-3300 editor route name.
pub const PS3300_EDITOR_ROUTE: &str = PS3300_EDITOR_ROUTE_NAME;
/// Korg PS-3300 editor view id.
pub const PS3300_EDITOR_VIEW: &str = PS3300_EDITOR_VIEW_ID;

/// Return Korg PS-3300 editor fixture names.
pub fn ps3300_editor_fixture_names() -> &'static [&'static str] {
    &PS3300_EDITOR_FIXTURE_NAMES
}

/// Build a deterministic Korg PS-3300 editor snapshot.
pub fn ps3300_editor_snapshot(name: &str) -> Option<Expr> {
    PS3300_EDITOR_FIXTURE_NAMES
        .contains(&name)
        .then(|| ps3300_editor_view(name))
}

/// Build the Korg PS-3300 panel editor view.
pub fn ps3300_editor_view(fixture_name: &str) -> Expr {
    let invalid = fixture_name == PS3300_EDITOR_FIXTURE_NAMES[2];
    let empty = fixture_name == PS3300_EDITOR_FIXTURE_NAMES[1];
    let representative = fixture_name == PS3300_EDITOR_FIXTURE_NAMES[3];
    let mut children = vec![
        action("poly-patch-load", "load-poly-patch", "Load poly patch"),
        section_panel(empty, representative),
        poly_array(empty),
        pin_matrix(representative),
        resonator_panel(representative),
        panel_graph(empty, representative),
    ];
    if invalid {
        children.push(validation_panel("illegal-pin-matrix-route", "pin-matrix"));
        children.push(validation_panel("missing-section-output", "section-c"));
    }
    editor_root(descriptor(), "ps3300-panel-editor", fixture_name, children)
}

fn descriptor() -> &'static InstrumentEditorDescriptor {
    INSTRUMENT_EDITOR_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.instrument == "ps3300")
        .expect("PS-3300 editor descriptor")
}

fn section_panel(empty: bool, representative: bool) -> Expr {
    let rows = if empty {
        Vec::new()
    } else {
        let mut rows = vec![
            section_row("section-a", 48.0, 0.8),
            section_row("section-b", 48.0, 0.6),
            section_row("section-c", 48.0, 0.5),
        ];
        if representative {
            rows.push(section_row("ensemble", 144.0, 0.7));
        }
        rows
    };
    table("section-editor", rows)
}

fn poly_array(empty: bool) -> Expr {
    let rows = if empty {
        vec![vec![0.0, 0.0, 0.0], vec![0.0, 0.0, 0.0]]
    } else {
        vec![vec![0.9, 0.6, 0.3], vec![0.4, 0.8, 0.5]]
    };
    grid(
        "poly-patch-panel",
        &rows,
        &["section-a", "section-b"],
        &["tone", "cell", "vca"],
    )
}

fn pin_matrix(representative: bool) -> Expr {
    let rows = if representative {
        vec![
            route_row("mod-gen", "section-a-cutoff", 0.3),
            route_row("sample-hold", "resonator-formant", 0.7),
            route_row("keyboard-cv", "tone-source", 1.0),
        ]
    } else {
        vec![
            route_row("keyboard-cv", "tone-source", 1.0),
            route_row("mod-gen", "section-a-cutoff", 0.2),
        ]
    };
    table("pin-matrix-editor", rows)
}

fn resonator_panel(representative: bool) -> Expr {
    let value = if representative { 0.72 } else { 0.45 };
    node(
        "stack",
        vec![
            ("role", sym("resonator-panel")),
            ("dir", sym("row")),
            (
                "children",
                Expr::List(vec![
                    slider("resonator-low", "low-formant", 0.0, 1.0, value),
                    slider("resonator-mid", "mid-formant", 0.0, 1.0, value * 0.8),
                    slider("resonator-high", "high-formant", 0.0, 1.0, value * 0.6),
                ]),
            ),
        ],
    )
}

fn panel_graph(empty: bool, representative: bool) -> Expr {
    if empty {
        return graph("ps3300-panel-graph", &[], &[]);
    }
    let mut nodes = vec![
        "keyboard",
        "tone-source",
        "section-a",
        "section-b",
        "section-c",
        "out",
    ];
    let mut edges = vec![
        ("keyboard", "tone-source"),
        ("tone-source", "section-a"),
        ("tone-source", "section-b"),
        ("tone-source", "section-c"),
        ("section-a", "out"),
        ("section-b", "out"),
        ("section-c", "out"),
    ];
    if representative {
        nodes.extend(["pin-matrix", "resonator"]);
        edges.push(("pin-matrix", "section-a"));
        edges.push(("resonator", "out"));
    }
    graph("ps3300-panel-graph", &nodes, &edges)
}

fn section_row(name: &str, voices: f64, level: f64) -> Expr {
    row(vec![
        ("section", Expr::String(name.to_owned())),
        ("voices", number(voices)),
        ("level", number(level)),
    ])
}

fn route_row(source: &str, target: &str, depth: f64) -> Expr {
    row(vec![
        ("source", Expr::String(source.to_owned())),
        ("target", Expr::String(target.to_owned())),
        ("depth", number(depth)),
    ])
}
