//! Shared builders for instrument-specific editor scenes.

use sim_kernel::Expr;
use sim_lib_music_synth::InstrumentEditorDescriptor;
use sim_lib_scene::{data_map, node, sym};

pub(crate) fn editor_root(
    descriptor: &InstrumentEditorDescriptor,
    role: &str,
    fixture_name: &str,
    children: Vec<Expr>,
) -> Expr {
    node(
        "stack",
        vec![
            ("lens", sym(descriptor.view_id)),
            ("role", sym(role)),
            ("route", sym(descriptor.route_name)),
            ("fixture", Expr::String(fixture_name.to_owned())),
            ("fixture-names", string_list(descriptor.fixture_names)),
            ("dir", sym("column")),
            ("children", Expr::List(children)),
        ],
    )
}

pub(crate) fn action(role: &str, action: &str, label: &str) -> Expr {
    node(
        "button",
        vec![
            ("role", sym(role)),
            ("action", sym(action)),
            ("label", Expr::String(label.to_owned())),
        ],
    )
}

pub(crate) fn slider(role: &str, param: &str, min: f64, max: f64, value: f64) -> Expr {
    node(
        "slider",
        vec![
            ("role", sym(role)),
            ("param", sym(param)),
            ("min", number(min)),
            ("max", number(max)),
            ("value", number(value)),
        ],
    )
}

pub(crate) fn table(role: &str, rows: Vec<Expr>) -> Expr {
    node(
        "table",
        vec![("role", sym(role)), ("rows", Expr::List(rows))],
    )
}

pub(crate) fn grid(
    role: &str,
    rows: &[Vec<f64>],
    row_labels: &[&str],
    col_labels: &[&str],
) -> Expr {
    node(
        "grid",
        vec![
            ("role", sym(role)),
            ("rows", numeric_rows(rows)),
            ("row-labels", string_list(row_labels)),
            ("col-labels", string_list(col_labels)),
        ],
    )
}

pub(crate) fn graph(role: &str, nodes: &[&str], edges: &[(&str, &str)]) -> Expr {
    node(
        "graph",
        vec![
            ("role", sym(role)),
            (
                "nodes",
                Expr::List(nodes.iter().map(|name| graph_node(name)).collect()),
            ),
            (
                "edges",
                Expr::List(
                    edges
                        .iter()
                        .map(|(from, to)| graph_edge(from, to))
                        .collect(),
                ),
            ),
        ],
    )
}

pub(crate) fn row(fields: Vec<(&str, Expr)>) -> Expr {
    data_map(fields)
}

pub(crate) fn validation_panel(code: &str, target: &str) -> Expr {
    node(
        "badge",
        vec![
            ("role", sym("instrument-editor-validation")),
            ("status", sym("error")),
            ("validation", sym(code)),
            ("target", sym(target)),
            ("label", Expr::String(format!("{target}: {code}"))),
        ],
    )
}

pub(crate) fn number_list(values: &[f64]) -> Expr {
    Expr::List(values.iter().map(|value| number(*value)).collect())
}

pub(crate) fn string_list(values: &[&str]) -> Expr {
    Expr::List(
        values
            .iter()
            .map(|value| Expr::String((*value).to_owned()))
            .collect(),
    )
}

pub(crate) fn numeric_rows(rows: &[Vec<f64>]) -> Expr {
    Expr::List(
        rows.iter()
            .map(|row| Expr::List(row.iter().map(|value| number(*value)).collect()))
            .collect(),
    )
}

pub(crate) use sim_value::build::float as number;

fn graph_node(name: &str) -> Expr {
    node(
        "node",
        vec![("id", sym(name)), ("label", Expr::String(name.to_owned()))],
    )
}

fn graph_edge(from: &str, to: &str) -> Expr {
    node("edge", vec![("from", sym(from)), ("to", sym(to))])
}
