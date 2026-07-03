//! Roland System 700 instrument editor scenes.

use sim_kernel::Expr;
use sim_lib_music_synth::{
    INSTRUMENT_EDITOR_DESCRIPTORS, InstrumentEditorDescriptor, SYSTEM700_EDITOR_FIXTURE_NAMES,
    SYSTEM700_EDITOR_ROUTE_NAME, SYSTEM700_EDITOR_VIEW_ID,
};
use sim_lib_scene::{node, sym};

use crate::instrument::{action, editor_root, graph, number, row, slider, table, validation_panel};

/// Roland System 700 editor route name.
pub const SYSTEM700_EDITOR_ROUTE: &str = SYSTEM700_EDITOR_ROUTE_NAME;
/// Roland System 700 editor view id.
pub const SYSTEM700_EDITOR_VIEW: &str = SYSTEM700_EDITOR_VIEW_ID;

/// Return Roland System 700 editor fixture names.
pub fn system700_editor_fixture_names() -> &'static [&'static str] {
    &SYSTEM700_EDITOR_FIXTURE_NAMES
}

/// Build a deterministic Roland System 700 editor snapshot.
pub fn system700_editor_snapshot(name: &str) -> Option<Expr> {
    SYSTEM700_EDITOR_FIXTURE_NAMES
        .contains(&name)
        .then(|| system700_editor_view(name))
}

/// Build the Roland System 700 panel editor view.
pub fn system700_editor_view(fixture_name: &str) -> Expr {
    let invalid = fixture_name == SYSTEM700_EDITOR_FIXTURE_NAMES[2];
    let empty = fixture_name == SYSTEM700_EDITOR_FIXTURE_NAMES[1];
    let representative = fixture_name == SYSTEM700_EDITOR_FIXTURE_NAMES[3];
    let mut children = vec![
        action("patch-load", "load-patch", "Load patch"),
        panel_modules(empty, representative),
        panel_graph(empty, representative),
        cord_editor(empty, representative),
        trace_panel(),
    ];
    if invalid {
        children.push(validation_panel("missing-vco-output", "r700-vco-1"));
        children.push(validation_panel("unpatched-audio-out", "main-out"));
    }
    editor_root(
        descriptor(),
        "system700-panel-editor",
        fixture_name,
        children,
    )
}

fn descriptor() -> &'static InstrumentEditorDescriptor {
    INSTRUMENT_EDITOR_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.instrument == "system700")
        .expect("System 700 editor descriptor")
}

fn panel_modules(empty: bool, representative: bool) -> Expr {
    let rows = if empty {
        Vec::new()
    } else {
        let mut rows = vec![
            module_row("r700-vco-1", "vco", 2.0),
            module_row("r700-vcf", "filter", 1.0),
            module_row("r700-vca", "amplifier", 1.0),
            module_row("r700-envelope", "envelope", 2.0),
            module_row("r700-mixer", "mixer", 1.0),
        ];
        if representative {
            rows.push(module_row("r700-sequencer", "sequencer", 1.0));
            rows.push(module_row("r700-clock", "clock", 1.0));
        }
        rows
    };
    table("modular-patch-panel", rows)
}

fn panel_graph(empty: bool, representative: bool) -> Expr {
    if empty {
        return graph("system700-panel-graph", &[], &[]);
    }
    let mut nodes = vec!["r700-vco-1", "r700-vcf", "r700-vca", "r700-envelope", "out"];
    let mut edges = vec![
        ("r700-vco-1", "r700-vcf"),
        ("r700-vcf", "r700-vca"),
        ("r700-vca", "out"),
        ("r700-envelope", "r700-vca"),
    ];
    if representative {
        nodes.extend(["r700-sequencer", "r700-clock"]);
        edges.push(("r700-clock", "r700-sequencer"));
        edges.push(("r700-sequencer", "r700-vco-1"));
    }
    graph("system700-panel-graph", &nodes, &edges)
}

fn cord_editor(empty: bool, representative: bool) -> Expr {
    let rows = if empty {
        Vec::new()
    } else {
        let mut rows = vec![
            cord_row("r700-vco-1.audio", "r700-vcf.audio-in"),
            cord_row("r700-vcf.audio", "r700-vca.audio-in"),
            cord_row("r700-envelope.cv", "r700-vca.cv"),
        ];
        if representative {
            rows.push(cord_row("r700-sequencer.cv", "r700-vco-1.pitch"));
        }
        rows
    };
    table("cord-editor", rows)
}

fn trace_panel() -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("system700-trace-view")),
            ("dir", sym("row")),
            (
                "children",
                Expr::List(vec![
                    slider("cv-trace", "sequencer-cv", -5.0, 5.0, 1.2),
                    slider("gate-trace", "gate-width", 0.0, 1.0, 0.5),
                ]),
            ),
        ],
    )
}

fn module_row(id: &str, role: &str, count: f64) -> Expr {
    row(vec![
        ("module", Expr::String(id.to_owned())),
        ("role", Expr::String(role.to_owned())),
        ("count", number(count)),
    ])
}

fn cord_row(from: &str, to: &str) -> Expr {
    row(vec![
        ("from", Expr::String(from.to_owned())),
        ("to", Expr::String(to.to_owned())),
    ])
}
