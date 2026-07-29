//! Deterministic cookbook builders for math view recipes.

use sim_kernel::Expr;

use crate::{HeatmapData, VIRIDIS_PALETTE, heatmap_view, plot_view};

/// Build the domain-neutral masked heatmap Scene used by the cookbook recipe.
pub fn heatmap_grid_demo() -> Expr {
    let caps = sim_lib_view::SurfaceCaps::from_preset("desktop", "cookbook.heatmap")
        .expect("desktop surface preset exists");
    let scene = heatmap_view(
        HeatmapData {
            rows: 2,
            cols: 3,
            values: &[0.0, 0.2, 0.4, 0.6, 0.8, 1.0],
            valid: &[true, true, false, true, true, true],
            range: (0.0, 1.0),
            palette: VIRIDIS_PALETTE,
            label: "Normalized scalar field",
            detector: "caller-prepared point samples",
            advisory: Some("masked cells are unavailable"),
        },
        &caps,
    )
    .expect("static cookbook heatmap is within the desktop budget");
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

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
    fn heatmap_grid_demo_is_a_valid_scene() {
        sim_lib_scene::validate_scene(&heatmap_grid_demo()).expect("heatmap scene validates");
    }

    #[test]
    fn plot_series_demo_is_a_valid_scene() {
        sim_lib_scene::validate_scene(&plot_series_demo()).expect("plot scene validates");
    }
}
