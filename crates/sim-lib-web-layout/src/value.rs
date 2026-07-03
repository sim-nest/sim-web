//! The workspace value and small map helpers.
//!
//! The whole workspace is one SIM value: a map of panes, the focused pane, the
//! experience mode, the session and history refs, and palette state. It is
//! built from kernel `Expr` only, so it round-trips through any general codec
//! and can be saved, shared, versioned, diffed, and restored as data. Layout is
//! data; restoring a session is decoding a value.

use sim_kernel::{Expr, Symbol};

// The generic `Expr` constructors and accessors live in `sim-value`; these
// re-exports preserve this module's public names and signatures so consumers
// (`palette`, `layout`, ...) keep calling `value::int`/`map`/`key`/`get`/`set`/
// `as_int` unchanged.
pub use sim_value::access::as_i64 as as_int;
pub use sim_value::access::field as get;
pub use sim_value::access::set;
pub use sim_value::build::sym as key;
pub use sim_value::build::{int, map};

/// The workspace class symbol carried in the `class` field.
pub const WORKSPACE_CLASS: &str = "web/Workspace";

/// A fresh, empty workspace in `mode` with no panes.
pub fn new_workspace(mode: &str) -> Expr {
    map(vec![
        ("class", Expr::Symbol(Symbol::qualified("web", "Workspace"))),
        ("mode", Expr::Symbol(Symbol::new(mode))),
        ("panes", Expr::List(Vec::new())),
        ("focus", Expr::Nil),
        ("session", Expr::Nil),
        ("history", Expr::Nil),
        (
            "palette",
            map(vec![
                ("open", Expr::Bool(false)),
                ("query", Expr::String(String::new())),
            ]),
        ),
    ])
}

/// The workspace mode symbol, if set.
pub fn mode(workspace: &Expr) -> Option<Symbol> {
    match get(workspace, "mode") {
        Some(Expr::Symbol(symbol)) => Some(symbol.clone()),
        _ => None,
    }
}

/// The list of pane records.
pub fn panes(workspace: &Expr) -> Vec<Expr> {
    match get(workspace, "panes") {
        Some(Expr::List(items)) => items.clone(),
        _ => Vec::new(),
    }
}

/// The focused pane id, if any.
pub fn focus(workspace: &Expr) -> Option<Symbol> {
    match get(workspace, "focus") {
        Some(Expr::Symbol(symbol)) => Some(symbol.clone()),
        _ => None,
    }
}

/// Set the panes list.
pub fn with_panes(workspace: &Expr, panes: Vec<Expr>) -> Expr {
    set(workspace, "panes", Expr::List(panes))
}
