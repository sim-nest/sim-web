//! Numeric helpers shared by the math lenses.
//!
//! The math lenses display values from the numeric domains (`sim-lib-numbers-*`)
//! by reading their canonical literal as `f64` for layout. The runtime value
//! stays the authoritative number; this is a display projection only.

use sim_kernel::{Expr, NumberLiteral, Symbol};

/// Build an `f64`-domain number value.
pub fn number(value: f64) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::new("f64"),
        canonical: format_f64(value),
    })
}

/// Read a number value's canonical literal as `f64`, if it is a number.
pub fn as_f64(value: &Expr) -> Option<f64> {
    match value {
        Expr::Number(number) => number.canonical.parse::<f64>().ok(),
        _ => None,
    }
}

/// Format an `f64` canonically (integers without a trailing `.0` noise).
pub fn format_f64(value: f64) -> String {
    if value.fract() == 0.0 && value.is_finite() {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// Build a 2D point value `[x, y]`.
pub fn point(x: f64, y: f64) -> Expr {
    Expr::Vector(vec![number(x), number(y)])
}
