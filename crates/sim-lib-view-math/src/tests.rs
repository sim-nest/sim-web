//! Tests for the math lens family.
//!
//! conformance: bounded heatmap surfaces preserve caller-prepared scalar grids,
//! masks, detector metadata, palettes, and display-derived refusal budgets.

use sim_kernel::Expr;

use crate::heatmap::{
    BLUE_RED_PALETTE, CYCLIC_PHASE_PALETTE, HeatmapData, VIRIDIS_PALETTE, heatmap_budget,
    heatmap_view,
};
use crate::matrix::{cell, labeled_matrix_view, matrix, matrix_view, set_cell};
use crate::plot::{plot_view, response_plot_view};
use crate::sweep::{Sweep, SweepParam, response_sweep_view};
use crate::symbolic::{call, symbolic_tree};

use sim_value::build::sym;

fn set_param_intent(value: f64) -> Expr {
    sim_lib_intent::intent(
        "set-param",
        sim_lib_intent::Origin::human(1),
        vec![
            ("target", sym("plot")),
            ("param", sym("slope")),
            ("value", crate::num::number(value)),
        ],
    )
}

#[test]
fn a_series_opens_in_a_plot_lens() {
    let scene = plot_view("y = x^2", &[(0.0, 0.0), (1.0, 1.0), (2.0, 4.0), (3.0, 9.0)]);
    sim_lib_scene::validate_scene(&scene).expect("the plot is a valid scene");
}

fn scalar_grid<'a>(values: &'a [f64], valid: &'a [bool], palette: &'a str) -> HeatmapData<'a> {
    HeatmapData {
        rows: 2,
        cols: 3,
        values,
        valid,
        range: (-1.0, 1.0),
        palette,
        label: "Signed scalar field",
        detector: "caller-prepared point samples",
        advisory: Some("one cell is unavailable"),
    }
}

#[test]
fn heatmap_view_preserves_caller_prepared_grid_without_downsampling() {
    let values = [-1.0, -0.5, 0.0, 0.25, 0.5, 1.0];
    let valid = [true, true, false, true, true, true];
    let caps =
        sim_lib_view::SurfaceCaps::from_preset("desktop", "heatmap.desktop").expect("desktop caps");
    let scene = heatmap_view(scalar_grid(&values, &valid, VIRIDIS_PALETTE), &caps)
        .expect("bounded grid projects");
    sim_lib_scene::validate_scene(&scene).expect("projected heatmap is a valid Scene");
    assert_eq!(number_values(&scene), values);
    assert_eq!(bool_values(&scene), valid);
    assert_eq!(field(&scene, "rows"), Some(sim_value::build::uint(2)));
    assert_eq!(field(&scene, "cols"), Some(sim_value::build::uint(3)));

    let short = heatmap_view(scalar_grid(&values[..5], &valid, VIRIDIS_PALETTE), &caps)
        .expect_err("mismatched values must fail before projection");
    assert!(short.to_string().contains("values has 5 entries"));
}

#[test]
fn heatmap_budget_uses_display_density_pixels_and_byte_limits() {
    let desktop =
        sim_lib_view::SurfaceCaps::from_preset("desktop", "heatmap.desktop").expect("desktop caps");
    let desktop_budget = heatmap_budget(&desktop).expect("desktop budget");
    assert_eq!(desktop_budget.max_cells(), 1024 * 1024);
    assert_eq!(desktop_budget.max_bytes(), 16 * 1024 * 1024);

    let watch =
        sim_lib_view::SurfaceCaps::from_preset("watch", "heatmap.watch").expect("watch caps");
    let watch_budget = heatmap_budget(&watch).expect("watch budget");
    assert_eq!(watch_budget.max_cells(), 4 * 1024);
    assert_eq!(watch_budget.max_bytes(), 64 * 1024);

    let mut pixel_limited = desktop.clone();
    pixel_limited.display = sim_value::access::set(
        &pixel_limited.display,
        "px",
        Expr::List(vec![sim_value::build::uint(8), sim_value::build::uint(4)]),
    );
    let pixel_budget = heatmap_budget(&pixel_limited).expect("pixel-derived budget");
    assert_eq!(pixel_budget.max_cells(), 32);

    let oversized_values = vec![0.0; watch_budget.max_cells() + 1];
    let oversized_valid = vec![true; watch_budget.max_cells() + 1];
    let error = heatmap_view(
        HeatmapData {
            rows: 1,
            cols: oversized_values.len(),
            values: &oversized_values,
            valid: &oversized_valid,
            range: (0.0, 0.0),
            palette: VIRIDIS_PALETTE,
            label: "Oversized",
            detector: "caller-prepared",
            advisory: None,
        },
        &watch,
    )
    .expect_err("oversized caller data must be refused");
    assert!(
        error
            .to_string()
            .contains("apply the domain detector rule first")
    );

    let mut byte_limited = desktop.clone();
    byte_limited.display = sim_value::access::set(
        &byte_limited.display,
        "heatmap-max-bytes",
        sim_value::build::uint(64),
    );
    let values = [0.0; 6];
    let valid = [true; 6];
    let error = heatmap_view(scalar_grid(&values, &valid, VIRIDIS_PALETTE), &byte_limited)
        .expect_err("oversized payload bytes must be refused");
    assert!(error.to_string().contains("64 bytes"));
}

#[test]
fn all_named_heatmap_palettes_are_data_and_unknown_names_fail_closed() {
    let values = [-1.0, -0.5, 0.0, 0.25, 0.5, 1.0];
    let valid = [true, true, false, true, true, true];
    let caps =
        sim_lib_view::SurfaceCaps::from_preset("desktop", "heatmap.desktop").expect("desktop caps");
    for palette in [VIRIDIS_PALETTE, BLUE_RED_PALETTE, CYCLIC_PHASE_PALETTE] {
        let scene = heatmap_view(scalar_grid(&values, &valid, palette), &caps)
            .unwrap_or_else(|error| panic!("{palette}: {error}"));
        assert_eq!(
            field(&scene, "palette"),
            Some(sim_value::build::sym(palette))
        );
    }

    let error = heatmap_view(scalar_grid(&values, &valid, "rainbow"), &caps)
        .expect_err("unknown palette must fail closed");
    assert!(
        error
            .to_string()
            .contains("palette 'rainbow' is not recognized")
    );
}

#[test]
fn a_tensor_opens_in_an_editable_matrix_lens() {
    let value = matrix(&[vec![1.0, 2.0], vec![3.0, 4.0]]);
    let scene = matrix_view(&value);
    sim_lib_scene::validate_scene(&scene).expect("the matrix is a valid scene");
    // Editing a cell returns a new matrix value.
    let edited = set_cell(&value, 1, 0, 9.0);
    assert_eq!(cell(&edited, 1, 0), Some(9.0));
    assert_eq!(
        cell(&edited, 0, 1),
        Some(2.0),
        "sibling cells are preserved"
    );
    assert_eq!(cell(&value, 1, 0), Some(3.0), "the original is unchanged");
}

#[test]
fn labelled_response_helpers_validate_as_component_views() {
    let route = matrix(&[vec![0.0, 1.0], vec![0.5, 0.0]]);
    let matrix_scene = labeled_matrix_view(
        "view:test-routing",
        "algorithm-routing-matrix",
        &route,
        &["op1", "op2"],
        &["op1", "out"],
    );
    sim_lib_scene::validate_scene(&matrix_scene).expect("the labelled matrix is valid");
    assert_eq!(
        field(&matrix_scene, "role"),
        Some(sym("algorithm-routing-matrix"))
    );

    let series = [(
        "low-pass".to_owned(),
        vec![(20.0, 0.0), (200.0, -0.5), (2_000.0, -12.0)],
    )];
    let plot_scene = response_plot_view("view:test-response", "filter-response-plot", &series);
    sim_lib_scene::validate_scene(&plot_scene).expect("the response plot is valid");
    assert_eq!(
        field(&plot_scene, "role"),
        Some(sym("filter-response-plot"))
    );

    let sweep_scene = response_sweep_view(
        "view:test-sweep",
        "filter-response-view",
        SweepParam {
            name: "cutoff",
            min: 20.0,
            max: 20_000.0,
            value: 1_000.0,
        },
        &series,
    );
    sim_lib_scene::validate_scene(&sweep_scene).expect("the response sweep is valid");
    assert_eq!(
        field(&sweep_scene, "role"),
        Some(sym("filter-response-view"))
    );
}

#[test]
fn a_symbolic_expression_opens_in_a_tree_lens() {
    // a*x + b
    let expr = call("+", vec![call("*", vec![sym("a"), sym("x")]), sym("b")]);
    let scene = symbolic_tree(&expr);
    sim_lib_scene::validate_scene(&scene).expect("the symbolic tree is a valid scene");
}

#[test]
fn a_parameter_sweep_updates_the_plot_live() {
    let mut sweep = Sweep::new(1.0, 5);
    let first = sweep.plot();
    sim_lib_scene::validate_scene(&first).expect("the sweep plot is valid");

    // Driving the parameter updates the plot.
    let updated = sweep.set_param(&set_param_intent(3.0)).unwrap();
    sim_lib_scene::validate_scene(&updated).expect("the updated plot is valid");
    assert_eq!(sweep.param(), 3.0);
    assert_ne!(first, updated, "the plot changed with the parameter");
}

#[test]
fn snapshots_compare_several_parameter_settings() {
    let mut sweep = Sweep::new(1.0, 5);
    sweep.snapshot();
    sweep.set_param(&set_param_intent(2.0)).unwrap();
    sweep.snapshot();
    sweep.set_param(&set_param_intent(3.0)).unwrap();
    assert_eq!(sweep.snapshot_count(), 2);
    let compare = sweep.compare();
    sim_lib_scene::validate_scene(&compare).expect("the compare plot is valid");
    // Two snapshots plus the current series overlay in one plot.
    assert!(series_count(&compare) >= 3);
}

#[test]
fn set_param_rejects_other_intents() {
    let mut sweep = Sweep::new(1.0, 5);
    let other = sim_lib_intent::intent(
        "commit",
        sim_lib_intent::Origin::human(1),
        vec![("pane", sym("p"))],
    );
    assert!(sweep.set_param(&other).is_err());
}

fn series_count(plot: &Expr) -> usize {
    let Expr::Map(entries) = plot else { return 0 };
    entries
        .iter()
        .find_map(|(key, value)| {
            let is_series = matches!(key, Expr::Symbol(s) if &*s.name == "series");
            match value {
                Expr::List(items) if is_series => Some(items.len()),
                _ => None,
            }
        })
        .unwrap_or(0)
}

fn field(map: &Expr, name: &str) -> Option<Expr> {
    let Expr::Map(entries) = map else { return None };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(s) if &*s.name == name).then(|| value.clone())
    })
}

fn number_values(scene: &Expr) -> Vec<f64> {
    match field(scene, "values") {
        Some(Expr::List(values)) => values
            .iter()
            .map(|value| crate::num::as_f64(value).expect("number"))
            .collect(),
        _ => Vec::new(),
    }
}

fn bool_values(scene: &Expr) -> Vec<bool> {
    match field(scene, "valid") {
        Some(Expr::List(values)) => values
            .into_iter()
            .map(|value| match value {
                Expr::Bool(value) => value,
                other => panic!("expected bool, got {other:?}"),
            })
            .collect(),
        _ => Vec::new(),
    }
}
