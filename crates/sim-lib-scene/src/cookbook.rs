//! Deterministic cookbook builders for Scene recipes.

use sim_kernel::Expr;

use crate::{badge, box_, stack, text_node, validate_scene};

/// Build the Scene used by the text-node cookbook recipe.
pub fn text_node_demo() -> Expr {
    let scene = stack(
        "column",
        vec![box_(
            "scene-text-demo",
            vec![text_node("Hello from a Scene value"), badge("ok", "scene")],
        )],
    );
    debug_assert!(validate_scene(&scene).is_ok());
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_node_demo_is_a_valid_scene() {
        validate_scene(&text_node_demo()).expect("demo scene validates");
    }
}
