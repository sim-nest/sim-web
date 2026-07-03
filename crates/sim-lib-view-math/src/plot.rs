//! Plotting lenses: 2D series and function plots as `scene/plot`.
//!
//! A series is a list of 2D points; a plot is axes plus one or more named
//! series. Multiple series overlay for snapshot-and-compare.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};

use crate::num::{number, point};

/// The plotting lens id.
pub const PLOT_LENS: &str = "view:math-plot";

/// Build a series value from `(x, y)` points.
pub fn series(name: &str, points: &[(f64, f64)]) -> Expr {
    data_map(vec![
        ("name", Expr::Symbol(Symbol::new(name))),
        (
            "points",
            Expr::List(points.iter().map(|(x, y)| point(*x, *y)).collect()),
        ),
    ])
}

/// A `scene/plot` of a single named series.
pub fn plot_view(name: &str, points: &[(f64, f64)]) -> Expr {
    multi_plot_view(&[(name.to_owned(), points.to_vec())])
}

/// A `scene/plot` overlaying several named series (snapshot and compare).
pub fn multi_plot_view(series_list: &[(String, Vec<(f64, f64)>)]) -> Expr {
    let bounds = bounds_of(series_list);
    let series = series_list
        .iter()
        .map(|(name, points)| {
            // Use `style`, not `kind`: `kind` is the scene-node tag. The
            // reserved-key guard in `data_map` enforces that.
            data_map(vec![
                ("name", Expr::Symbol(Symbol::new(name.as_str()))),
                ("style", sym("line")),
                (
                    "points",
                    Expr::List(points.iter().map(|(x, y)| point(*x, *y)).collect()),
                ),
            ])
        })
        .collect();
    node(
        "plot",
        vec![
            (
                "axes",
                data_map(vec![
                    ("x", axis(bounds.0, bounds.1)),
                    ("y", axis(bounds.2, bounds.3)),
                ]),
            ),
            ("series", Expr::List(series)),
        ],
    )
}

/// A named response plot for component-specific lenses.
pub fn response_plot_view(
    lens: &str,
    role: &str,
    series_list: &[(String, Vec<(f64, f64)>)],
) -> Expr {
    let mut plot = multi_plot_view(series_list);
    if let Expr::Map(entries) = &mut plot {
        entries.push((sym("lens"), Expr::Symbol(Symbol::new(lens))));
        entries.push((sym("role"), sym(role)));
    }
    plot
}

fn axis(min: f64, max: f64) -> Expr {
    data_map(vec![("min", number(min)), ("max", number(max))])
}

fn bounds_of(series_list: &[(String, Vec<(f64, f64)>)]) -> (f64, f64, f64, f64) {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for (_, points) in series_list {
        for (x, y) in points {
            bounds = Some(match bounds {
                Some((min_x, max_x, min_y, max_y)) => {
                    (min_x.min(*x), max_x.max(*x), min_y.min(*y), max_y.max(*y))
                }
                None => (*x, *x, *y, *y),
            });
        }
    }
    bounds.unwrap_or((0.0, 1.0, 0.0, 1.0))
}
