//! Math, plotting, tensor, and symbolic lenses for the SIM Web-UI (WEBUI_4).
//!
//! This lens family makes graphical math first-class: `scene/plot` series and
//! function plots, `scene/matrix` editable tensor/matrix slices, a
//! symbolic-expression tree lens, and slider/knob-driven parameter sweeps
//! (`intent/set-param`) with snapshot and compare. Numbers are read from the
//! existing `sim-lib-numbers-*` domains for display; the runtime value stays the
//! authoritative number.
//!
//! # Example
//!
//! A numeric series opens in a plot lens as a Scene value:
//!
//! ```
//! let scene = sim_lib_view_math::plot_view("y = x", &[(0.0, 0.0), (1.0, 1.0)]);
//! assert!(sim_lib_scene::validate_scene(&scene).is_ok());
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod matrix;
pub mod num;
pub mod plot;
pub mod sweep;
pub mod symbolic;

pub use matrix::{MATRIX_LENS, cell, matrix, matrix_view, set_cell};
pub use num::{as_f64, number, point};
pub use plot::{PLOT_LENS, multi_plot_view, plot_view, series};
pub use sweep::Sweep;
pub use symbolic::{SYMBOLIC_LENS, call, symbolic_tree};

#[cfg(test)]
mod tests;
