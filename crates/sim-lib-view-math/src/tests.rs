//! Tests for the math lens family.

use sim_kernel::Expr;

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
