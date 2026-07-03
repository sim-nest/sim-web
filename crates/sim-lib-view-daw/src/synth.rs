//! The synth and signal lenses: parameter panels, modulation matrix, waveform,
//! and spectrum.
//!
//! A synth patch is a parameter map (name -> value). The panel renders each
//! parameter as a `scene/knob`, plus a modulation matrix (`scene/matrix`) and
//! live signal displays (`scene/waveform`, `scene/spectrum`). Parameter changes
//! flow through `intent/set-param` (see `param`).

use sim_kernel::Expr;
use sim_lib_scene::{node, sym};

/// The synth panel lens id.
pub const SYNTH_LENS: &str = "view:daw-synth";

/// Render a synth parameter map as a knob panel plus a modulation matrix.
pub fn synth_panel(params: &Expr) -> Expr {
    let knobs = match params {
        Expr::Map(entries) => entries
            .iter()
            .map(|(key, value)| knob(key, value))
            .collect(),
        _ => Vec::new(),
    };
    node(
        "stack",
        vec![
            ("role", sym("synth")),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![
                    node(
                        "stack",
                        vec![
                            ("role", sym("knobs")),
                            ("dir", sym("row")),
                            ("children", Expr::List(knobs)),
                        ],
                    ),
                    modulation_matrix(&[vec![0.0, 0.0], vec![0.0, 0.0]]),
                ]),
            ),
        ],
    )
}

fn knob(name: &Expr, value: &Expr) -> Expr {
    node(
        "knob",
        vec![
            ("param", name.clone()),
            ("min", number(0.0)),
            ("max", number(1.0)),
            ("value", value.clone()),
        ],
    )
}

/// A modulation matrix as an editable `scene/matrix`.
pub fn modulation_matrix(rows: &[Vec<f64>]) -> Expr {
    let rows = rows
        .iter()
        .map(|row| Expr::List(row.iter().map(|v| number(*v)).collect()))
        .collect();
    node(
        "matrix",
        vec![
            ("role", sym("modulation")),
            ("rows", Expr::List(rows)),
            ("editable", Expr::Bool(true)),
        ],
    )
}

/// A sampled-signal display.
pub fn waveform_view(samples: &[f32]) -> Expr {
    node(
        "waveform",
        vec![(
            "samples",
            Expr::List(samples.iter().map(|s| number(*s as f64)).collect()),
        )],
    )
}

/// A frequency-domain display.
pub fn spectrum_view(bins: &[f32]) -> Expr {
    node(
        "spectrum",
        vec![(
            "bins",
            Expr::List(bins.iter().map(|b| number(*b as f64)).collect()),
        )],
    )
}

use sim_value::build::float as number;
