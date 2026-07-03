//! The layout engine: layout operations over the workspace value.
//!
//! Layout operations create, move, resize, dock, undock, and close panes. Each
//! produces a new workspace value; nothing is mutated in place, so the result is
//! still a round-trippable value. Operations arrive either directly (as a
//! [`LayoutOp`]) or as Intents the bridge has already validated.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_intent::{field, intent_kind_of};

use crate::pane::{new_pane, pane_id};
use crate::value::{get, int, set, with_panes};

/// A layout command over the workspace.
#[derive(Clone, Debug)]
pub enum LayoutOp {
    /// Open a resource into a new pane.
    Open {
        /// New pane id.
        id: Symbol,
        /// The resource value to show.
        resource: Expr,
        /// The initial lens id.
        lens: Symbol,
        /// The dock to place it in.
        dock: Symbol,
    },
    /// Close a pane.
    Close {
        /// The pane to remove.
        id: Symbol,
    },
    /// Move a pane's top-left corner.
    Move {
        /// The pane to move.
        id: Symbol,
        /// New x.
        x: i64,
        /// New y.
        y: i64,
    },
    /// Resize a pane.
    Resize {
        /// The pane to resize.
        id: Symbol,
        /// New width.
        w: i64,
        /// New height.
        h: i64,
    },
    /// Dock a pane to a region.
    Dock {
        /// The pane to dock.
        id: Symbol,
        /// The dock region symbol.
        dock: Symbol,
    },
    /// Undock a pane (float it).
    Undock {
        /// The pane to float.
        id: Symbol,
    },
}

/// Apply a layout operation, returning the updated workspace value.
pub fn apply_layout_op(workspace: &Expr, op: &LayoutOp) -> Result<Expr> {
    match op {
        LayoutOp::Open {
            id,
            resource,
            lens,
            dock,
        } => {
            if find_pane(workspace, id).is_some() {
                return Err(Error::HostError(format!("pane '{id}' already exists")));
            }
            let pane = new_pane(&id.name, resource.clone(), &lens.name, &dock.name);
            let mut panes = crate::value::panes(workspace);
            panes.push(pane);
            Ok(set(
                &with_panes(workspace, panes),
                "focus",
                Expr::Symbol(id.clone()),
            ))
        }
        LayoutOp::Close { id } => {
            let panes = crate::value::panes(workspace)
                .into_iter()
                .filter(|pane| pane_id(pane).as_ref() != Some(id))
                .collect();
            let updated = with_panes(workspace, panes);
            let focus_cleared = match get(&updated, "focus") {
                Some(Expr::Symbol(focus)) if focus == id => set(&updated, "focus", Expr::Nil),
                _ => updated,
            };
            Ok(focus_cleared)
        }
        LayoutOp::Move { id, x, y } => update_pane(workspace, id, |pane| {
            let rect = set(get(pane, "rect").unwrap_or(&Expr::Nil), "x", int(*x));
            let rect = set(&rect, "y", int(*y));
            set(pane, "rect", rect)
        }),
        LayoutOp::Resize { id, w, h } => update_pane(workspace, id, |pane| {
            let rect = set(get(pane, "rect").unwrap_or(&Expr::Nil), "w", int(*w));
            let rect = set(&rect, "h", int(*h));
            set(pane, "rect", rect)
        }),
        LayoutOp::Dock { id, dock } => update_pane(workspace, id, |pane| {
            set(pane, "dock", Expr::Symbol(dock.clone()))
        }),
        LayoutOp::Undock { id } => update_pane(workspace, id, |pane| {
            set(pane, "dock", Expr::Symbol(Symbol::new("float")))
        }),
    }
}

fn find_pane(workspace: &Expr, id: &Symbol) -> Option<Expr> {
    crate::value::panes(workspace)
        .into_iter()
        .find(|pane| pane_id(pane).as_ref() == Some(id))
}

fn update_pane(workspace: &Expr, id: &Symbol, edit: impl Fn(&Expr) -> Expr) -> Result<Expr> {
    let mut found = false;
    let panes = crate::value::panes(workspace)
        .iter()
        .map(|pane| {
            if pane_id(pane).as_ref() == Some(id) {
                found = true;
                edit(pane)
            } else {
                pane.clone()
            }
        })
        .collect();
    if !found {
        return Err(Error::HostError(format!("no pane '{id}' to update")));
    }
    Ok(with_panes(workspace, panes))
}

/// Translate a validated layout Intent into a [`LayoutOp`], if it is one.
///
/// `intent/open` opens a resource into a pane; `intent/invoke` with a layout
/// `op` (close/move/resize/dock/undock) carries the remaining ops. Returns
/// `Ok(None)` for an Intent that is not a layout command.
pub fn layout_op_from_intent(intent: &Expr) -> Result<Option<LayoutOp>> {
    let Some(kind) = intent_kind_of(intent) else {
        return Ok(None);
    };
    match &*kind.name {
        "open" => {
            let resource = field(intent, "value").cloned().unwrap_or(Expr::Nil);
            let id = require_symbol(intent, "pane")?;
            Ok(Some(LayoutOp::Open {
                id,
                resource,
                lens: Symbol::new("view:default"),
                dock: Symbol::new("center"),
            }))
        }
        "invoke" => layout_op_from_invoke(intent),
        _ => Ok(None),
    }
}

fn layout_op_from_invoke(intent: &Expr) -> Result<Option<LayoutOp>> {
    let op = match field(intent, "op") {
        Some(Expr::Symbol(symbol)) => symbol.name.to_string(),
        _ => return Ok(None),
    };
    let id = require_symbol(intent, "target")?;
    let args = field(intent, "args");
    let arg_int = |name: &str| {
        args.and_then(|args| get(args, name))
            .and_then(crate::value::as_int)
    };
    match op.as_str() {
        "close" => Ok(Some(LayoutOp::Close { id })),
        "undock" => Ok(Some(LayoutOp::Undock { id })),
        "dock" => {
            let dock = match args.and_then(|args| get(args, "dock")) {
                Some(Expr::Symbol(symbol)) => symbol.clone(),
                _ => Symbol::new("center"),
            };
            Ok(Some(LayoutOp::Dock { id, dock }))
        }
        "move" => Ok(Some(LayoutOp::Move {
            id,
            x: arg_int("x").unwrap_or(0),
            y: arg_int("y").unwrap_or(0),
        })),
        "resize" => Ok(Some(LayoutOp::Resize {
            id,
            w: arg_int("w").unwrap_or(0),
            h: arg_int("h").unwrap_or(0),
        })),
        _ => Ok(None),
    }
}

fn require_symbol(intent: &Expr, name: &str) -> Result<Symbol> {
    match field(intent, name) {
        Some(Expr::Symbol(symbol)) => Ok(symbol.clone()),
        _ => Err(Error::HostError(format!(
            "layout intent field '{name}' must be a symbol"
        ))),
    }
}
