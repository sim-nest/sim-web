//! Deterministic cookbook builders for agent view recipes.

use sim_kernel::Expr;
use sim_lib_topology::Graph;

use crate::composer_view;

/// Build the composer graph Scene used by the cookbook recipe.
pub fn composer_demo() -> Expr {
    let graph = Graph::minimal("cookbook-composer");
    let scene = composer_view(&graph);
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_demo_is_a_valid_scene_graph() {
        let scene = composer_demo();
        sim_lib_scene::validate_scene(&scene).expect("composer scene validates");
        assert!(sim_test_support::contains_kind(&scene, "graph"));
    }
}
