//! Rendering the workspace value to a layout Scene.
//!
//! The layout engine emits a Scene describing the dock/split arrangement of
//! panes. It is pure data built from baseline scene node kinds, so the browser
//! paints it like any other Scene and the layout is testable headlessly.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{node, sym};

use crate::pane::{pane_dock, pane_id, pane_lens};
use crate::value::{focus, mode, panes};

/// Encode the workspace value into a layout Scene.
pub fn workspace_scene(workspace: &Expr) -> Expr {
    let focused = focus(workspace);
    let children = panes(workspace)
        .iter()
        .map(|pane| pane_box(pane, focused.as_ref()))
        .collect();
    node(
        "stack",
        vec![
            ("role", sym("workspace")),
            ("dir", sym("row")),
            (
                "mode",
                mode(workspace).map(Expr::Symbol).unwrap_or(Expr::Nil),
            ),
            ("children", Expr::List(children)),
        ],
    )
}

fn pane_box(pane: &Expr, focused: Option<&Symbol>) -> Expr {
    let id = pane_id(pane).unwrap_or_else(|| Symbol::new("?"));
    let dock = pane_dock(pane).unwrap_or_else(|| Symbol::new("center"));
    let lens = pane_lens(pane).unwrap_or_else(|| Symbol::new("view:default"));
    let is_focused = focused == Some(&id);
    node(
        "box",
        vec![
            ("role", sym("pane")),
            ("id", Expr::Symbol(id.clone())),
            ("focused", Expr::Bool(is_focused)),
            (
                "children",
                Expr::List(vec![
                    node("text", vec![("text", Expr::String(format!("pane {id}")))]),
                    node(
                        "badge",
                        vec![
                            ("status", Expr::Symbol(dock.clone())),
                            ("label", Expr::String(dock.name.to_string())),
                        ],
                    ),
                    node(
                        "text",
                        vec![("text", Expr::String(format!("lens: {lens}")))],
                    ),
                ]),
            ),
        ],
    )
}
