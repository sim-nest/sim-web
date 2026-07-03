//! Moog System 55 instrument editor scenes.

use sim_kernel::Expr;
use sim_lib_music_synth::{
    INSTRUMENT_EDITOR_DESCRIPTORS, InstrumentEditorDescriptor, SYSTEM55_EDITOR_FIXTURE_NAMES,
    SYSTEM55_EDITOR_ROUTE_NAME, SYSTEM55_EDITOR_VIEW_ID,
};
use sim_lib_scene::{node, sym};

use crate::instrument::{action, editor_root, graph, grid, number, row, table, validation_panel};

/// Moog System 55 editor route name.
pub const SYSTEM55_EDITOR_ROUTE: &str = SYSTEM55_EDITOR_ROUTE_NAME;
/// Moog System 55 editor view id.
pub const SYSTEM55_EDITOR_VIEW: &str = SYSTEM55_EDITOR_VIEW_ID;

/// Return Moog System 55 editor fixture names.
pub fn system55_editor_fixture_names() -> &'static [&'static str] {
    &SYSTEM55_EDITOR_FIXTURE_NAMES
}

/// Build a deterministic Moog System 55 editor snapshot.
pub fn system55_editor_snapshot(name: &str) -> Option<Expr> {
    SYSTEM55_EDITOR_FIXTURE_NAMES
        .contains(&name)
        .then(|| system55_editor_view(name))
}

/// Build the Moog System 55 cabinet editor view.
pub fn system55_editor_view(fixture_name: &str) -> Expr {
    let invalid = fixture_name == SYSTEM55_EDITOR_FIXTURE_NAMES[2];
    let empty = fixture_name == SYSTEM55_EDITOR_FIXTURE_NAMES[1];
    let representative = fixture_name == SYSTEM55_EDITOR_FIXTURE_NAMES[3];
    let mut children = vec![
        action("cabinet-load", "load-cabinet", "Load cabinet"),
        cabinet_rows(empty, representative),
        cabinet_graph(empty, representative),
        filter_bank(representative),
        s_trigger_panel(),
    ];
    if invalid {
        children.push(validation_panel("s-trigger-gate-mismatch", "m55-911"));
        children.push(validation_panel("cabinet-row-overflow", "top-cabinet"));
    }
    editor_root(
        descriptor(),
        "system55-cabinet-editor",
        fixture_name,
        children,
    )
}

fn descriptor() -> &'static InstrumentEditorDescriptor {
    INSTRUMENT_EDITOR_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.instrument == "system55")
        .expect("System 55 editor descriptor")
}

fn cabinet_rows(empty: bool, representative: bool) -> Expr {
    let rows = if empty {
        Vec::new()
    } else {
        let mut rows = vec![
            module_row("m55-921a", "oscillator-driver", "top"),
            module_row("m55-921b-a", "oscillator", "top"),
            module_row("m55-904a", "low-pass-filter", "middle"),
            module_row("m55-902", "vca", "middle"),
            module_row("m55-911", "envelope", "bottom"),
        ];
        if representative {
            rows.push(module_row("m55-907", "fixed-filter-bank", "middle"));
            rows.push(module_row("m55-960", "sequential-controller", "bottom"));
        }
        rows
    };
    table("cabinet-row", rows)
}

fn cabinet_graph(empty: bool, representative: bool) -> Expr {
    if empty {
        return graph("system55-cabinet-graph", &[], &[]);
    }
    let mut nodes = vec![
        "m55-921a",
        "m55-921b-a",
        "m55-904a",
        "m55-902",
        "m55-911",
        "out",
    ];
    let mut edges = vec![
        ("m55-921a", "m55-921b-a"),
        ("m55-921b-a", "m55-904a"),
        ("m55-904a", "m55-902"),
        ("m55-902", "out"),
        ("m55-911", "m55-902"),
    ];
    if representative {
        nodes.extend(["m55-907", "m55-960"]);
        edges.push(("m55-904a", "m55-907"));
        edges.push(("m55-960", "m55-921a"));
    }
    graph("system55-cabinet-graph", &nodes, &edges)
}

fn filter_bank(representative: bool) -> Expr {
    let gain = if representative { 1.0 } else { 0.4 };
    grid(
        "fixed-filter-bank-editor",
        &[vec![gain, 0.2, -0.5, 0.8], vec![0.1, -0.3, 0.6, -0.2]],
        &["low-bands", "high-bands"],
        &["50hz", "120hz", "400hz", "1k2"],
    )
}

fn s_trigger_panel() -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("s-trigger-panel")),
            ("dir", sym("row")),
            (
                "children",
                Expr::List(vec![
                    action(
                        "s-trigger-convert",
                        "convert-s-trigger",
                        "Convert S-trigger",
                    ),
                    table(
                        "s-trigger-map",
                        vec![row(vec![
                            ("source", Expr::String("gate".to_owned())),
                            ("target", Expr::String("s-trigger".to_owned())),
                            ("inverted", Expr::Bool(true)),
                        ])],
                    ),
                ]),
            ),
        ],
    )
}

fn module_row(id: &str, role: &str, cabinet: &str) -> Expr {
    row(vec![
        ("module", Expr::String(id.to_owned())),
        ("role", Expr::String(role.to_owned())),
        ("cabinet", Expr::String(cabinet.to_owned())),
        ("slots", number(1.0)),
    ])
}
