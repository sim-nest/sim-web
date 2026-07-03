//! Tensor/matrix lens: an editable `scene/matrix`.
//!
//! A matrix value is a list of rows of numbers. The lens renders it as a
//! `scene/matrix` with editable cells; editing a cell applies an
//! `intent/edit-field` path `[row, col]` and returns a new matrix value.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{node, sym};

use crate::num::{as_f64, number};

/// The matrix lens id.
pub const MATRIX_LENS: &str = "view:math-matrix";

/// Build a matrix value from rows of numbers.
pub fn matrix(rows: &[Vec<f64>]) -> Expr {
    Expr::List(
        rows.iter()
            .map(|row| Expr::List(row.iter().map(|value| number(*value)).collect()))
            .collect(),
    )
}

/// Render a matrix value as an editable `scene/matrix`.
pub fn matrix_view(value: &Expr) -> Expr {
    let rows = matrix_rows(value);
    node(
        "matrix",
        vec![("rows", Expr::List(rows)), ("editable", Expr::Bool(true))],
    )
}

/// Render a labelled matrix for routing, algorithms, and component grids.
pub fn labeled_matrix_view(
    lens: &str,
    role: &str,
    value: &Expr,
    row_labels: &[&str],
    col_labels: &[&str],
) -> Expr {
    node(
        "matrix",
        vec![
            ("lens", Expr::Symbol(Symbol::new(lens))),
            ("role", sym(role)),
            ("rows", Expr::List(matrix_rows(value))),
            ("row-labels", string_list(row_labels)),
            ("col-labels", string_list(col_labels)),
            ("editable", Expr::Bool(true)),
        ],
    )
}

/// Set a matrix cell, returning the new matrix value. Out-of-range indices
/// leave the matrix unchanged.
pub fn set_cell(value: &Expr, row: usize, col: usize, new_value: f64) -> Expr {
    let Expr::List(rows) = value else {
        return value.clone();
    };
    let mut rows = rows.clone();
    if let Some(Expr::List(cells)) = rows.get_mut(row)
        && col < cells.len()
    {
        cells[col] = number(new_value);
    }
    Expr::List(rows)
}

/// Read a matrix cell as `f64`.
pub fn cell(value: &Expr, row: usize, col: usize) -> Option<f64> {
    let Expr::List(rows) = value else {
        return None;
    };
    match rows.get(row) {
        Some(Expr::List(cells)) => cells.get(col).and_then(as_f64),
        _ => None,
    }
}

/// The `[row, col]` edit-field path for a matrix cell, in the standard segment
/// form used by the universal editor.
pub fn cell_path(row: usize, col: usize) -> Expr {
    sim_value::path::Path::new().index(row).index(col).to_expr()
}

fn matrix_rows(value: &Expr) -> Vec<Expr> {
    match value {
        Expr::List(rows) => rows
            .iter()
            .map(|row| match row {
                Expr::List(cells) => Expr::List(cells.clone()),
                other => Expr::List(vec![other.clone()]),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn string_list(values: &[&str]) -> Expr {
    Expr::List(
        values
            .iter()
            .map(|value| Expr::String((*value).to_owned()))
            .collect(),
    )
}
