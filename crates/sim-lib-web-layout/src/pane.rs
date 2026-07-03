//! Pane records.
//!
//! A pane is a map value: its id, the resource it shows, the active lens, its
//! dock position, and its rectangle. Panes are plain data inside the workspace
//! value.

use sim_kernel::{Expr, Symbol};

use crate::value::{get, int, map};

/// Build a rectangle value.
pub fn rect(x: i64, y: i64, w: i64, h: i64) -> Expr {
    map(vec![
        ("x", int(x)),
        ("y", int(y)),
        ("w", int(w)),
        ("h", int(h)),
    ])
}

/// Build a pane record.
pub fn new_pane(id: &str, resource: Expr, lens: &str, dock: &str) -> Expr {
    map(vec![
        ("id", Expr::Symbol(Symbol::new(id))),
        ("resource", resource),
        ("lens", Expr::Symbol(Symbol::new(lens))),
        ("dock", Expr::Symbol(Symbol::new(dock))),
        ("rect", rect(0, 0, 400, 300)),
    ])
}

/// The pane id, if present.
pub fn pane_id(pane: &Expr) -> Option<Symbol> {
    match get(pane, "id") {
        Some(Expr::Symbol(symbol)) => Some(symbol.clone()),
        _ => None,
    }
}

/// The pane dock symbol, if present.
pub fn pane_dock(pane: &Expr) -> Option<Symbol> {
    match get(pane, "dock") {
        Some(Expr::Symbol(symbol)) => Some(symbol.clone()),
        _ => None,
    }
}

/// The pane's active lens, if present.
pub fn pane_lens(pane: &Expr) -> Option<Symbol> {
    match get(pane, "lens") {
        Some(Expr::Symbol(symbol)) => Some(symbol.clone()),
        _ => None,
    }
}

/// The pane resource value, if present.
pub fn pane_resource(pane: &Expr) -> Option<&Expr> {
    get(pane, "resource")
}

/// The pane rectangle, if present.
pub fn pane_rect(pane: &Expr) -> Option<&Expr> {
    get(pane, "rect")
}
