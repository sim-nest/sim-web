//! Deterministic cookbook builders for math view recipes.

use sim_kernel::Expr;

use crate::plot_view;

/// Build the plot Scene used by the cookbook recipe.
pub fn plot_series_demo() -> Expr {
    let scene = plot_view("y = x^2", &[(0.0, 0.0), (1.0, 1.0), (2.0, 4.0)]);
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_series_demo_is_a_valid_scene() {
        sim_lib_scene::validate_scene(&plot_series_demo()).expect("plot scene validates");
    }
}
