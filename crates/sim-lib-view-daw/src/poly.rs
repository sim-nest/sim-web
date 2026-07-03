//! Polyphonic section view for component builders.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::node;
use sim_value::build::{int, map, sym, text, vector};

/// Stable lens id for the polyphonic section view.
pub const POLY_SECTION_VIEW_ID: &str = "view:component-poly-sections";

/// Builds the polyphonic section table Scene from a sections descriptor.
pub fn poly_section_view(sections: &Expr) -> Expr {
    let rows = sequence(field_named(sections, "sections").unwrap_or(sections))
        .into_iter()
        .enumerate()
        .map(|(index, section)| section_row(&section, index))
        .collect();
    node(
        "table",
        vec![
            ("lens", sym(POLY_SECTION_VIEW_ID)),
            ("role", sym("poly-section-view")),
            ("rows", vector(rows)),
            (
                "actions",
                vector(vec![
                    sym("enable-section"),
                    sym("disable-section"),
                    sym("inspect"),
                ]),
            ),
        ],
    )
}

fn section_row(section: &Expr, index: usize) -> Expr {
    let id = field_named(section, "id")
        .cloned()
        .unwrap_or_else(|| Expr::Symbol(Symbol::new(format!("section-{index}"))));
    let label = field_str_named(section, "label")
        .map(text)
        .unwrap_or_else(|| text(expr_label(&id)));
    map(vec![
        ("id", id),
        ("label", label),
        (
            "enabled",
            field_named(section, "enabled")
                .cloned()
                .unwrap_or(Expr::Bool(true)),
        ),
        (
            "voices",
            field_named(section, "voices")
                .cloned()
                .unwrap_or_else(|| int(1)),
        ),
        (
            "clock",
            field_named(section, "clock")
                .cloned()
                .unwrap_or_else(|| sym("sample-clock")),
        ),
        (
            "rate",
            field_named(section, "rate")
                .cloned()
                .unwrap_or_else(|| sym("audio-rate")),
        ),
        (
            "actions",
            vector(vec![sym("enable-section"), sym("disable-section")]),
        ),
    ])
}

fn sequence(value: &Expr) -> Vec<Expr> {
    match value {
        Expr::List(items) | Expr::Vector(items) => items.clone(),
        Expr::Nil => Vec::new(),
        other => vec![other.clone()],
    }
}

fn field_named<'a>(expr: &'a Expr, name: &str) -> Option<&'a Expr> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries
        .iter()
        .find_map(|(key, value)| (key_name(key) == Some(name)).then_some(value))
}

fn field_str_named<'a>(expr: &'a Expr, name: &str) -> Option<&'a str> {
    match field_named(expr, name) {
        Some(Expr::String(text)) => Some(text),
        _ => None,
    }
}

fn key_name(key: &Expr) -> Option<&str> {
    match key {
        Expr::Symbol(symbol) => Some(symbol.name.as_ref()),
        Expr::String(text) => Some(text),
        _ => None,
    }
}

fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::Symbol(symbol) => symbol.as_qualified_str(),
        Expr::String(text) => text.clone(),
        other => format!("{other:?}"),
    }
}
