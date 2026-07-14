//! Deterministic cookbook builders for web layout recipes.

use sim_kernel::{Expr, Symbol};

use crate::{LayoutOp, apply_layout_op, new_workspace, workspace_scene};

/// Build the workspace-pane Scene used by the cookbook recipe.
pub fn workspace_pane_demo() -> Expr {
    let workspace = apply_layout_op(
        &new_workspace("builder"),
        &LayoutOp::Open {
            id: Symbol::new("pane-main"),
            resource: Expr::Symbol(Symbol::qualified("resource", "article")),
            lens: Symbol::new("view:default"),
            dock: Symbol::new("center"),
        },
    )
    .expect("workspace demo opens pane");
    let scene = workspace_scene(&workspace);
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_pane_demo_is_a_valid_scene() {
        sim_lib_scene::validate_scene(&workspace_pane_demo()).expect("workspace scene validates");
    }
}
