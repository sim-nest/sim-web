//! Hardware-free worn capability fixtures for watch bring-up.

use sim_kernel::{Expr, Symbol};
use sim_value::build;

use crate::RateClass;

/// Namespace for worn capability fixture expressions.
pub const WORN_CAPS_NAMESPACE: &str = "worn";

/// Kind tag for worn capability fixture expressions.
pub const WORN_CAPS_KIND: &str = "caps";

/// Baseline fixture name for the 48mm Amazfit T-Rex 3 Pro.
pub const T_REX_3_PRO_48_CAPS_FIXTURE: &str = "trex3pro-48";

/// Returns the built-in worn capability fixture names.
pub fn worn_caps_fixture_names() -> [&'static str; 1] {
    [T_REX_3_PRO_48_CAPS_FIXTURE]
}

/// Returns the named worn capability fixture.
pub fn worn_caps_fixture(name: &str) -> Option<Expr> {
    match name {
        T_REX_3_PRO_48_CAPS_FIXTURE => Some(trex3pro_48_worn_caps_fixture()),
        _ => None,
    }
}

/// Builds the verified-baseline fixture for the 48mm Amazfit T-Rex 3 Pro.
///
/// Claims record vendor-stated capabilities. Verified flags are false in this
/// hardware-free baseline; hardware bring-up records firmware evidence before
/// changing them.
pub fn trex3pro_48_worn_caps_fixture() -> Expr {
    build::map(vec![
        (
            "kind",
            Expr::Symbol(Symbol::qualified(WORN_CAPS_NAMESPACE, WORN_CAPS_KIND)),
        ),
        ("device", build::sym(T_REX_3_PRO_48_CAPS_FIXTURE)),
        (
            "claims",
            build::map(vec![
                ("size-mm", build::uint(48)),
                (
                    "display-px",
                    build::list(vec![build::uint(480), build::uint(480)]),
                ),
                ("keys", build::uint(4)),
                ("ble-5-2", Expr::Bool(true)),
                ("wifi-2-4", Expr::Bool(true)),
                ("zepp-api", build::text("4.2")),
                ("zepp-os", build::text("5.0")),
                ("ble-hr", Expr::Bool(true)),
                ("notification-out", Expr::Bool(true)),
                ("mini-program", Expr::Bool(true)),
                ("mic", Expr::Bool(true)),
                ("speaker", Expr::Bool(true)),
            ]),
        ),
        (
            "verified",
            build::map(vec![
                ("ble-hr", Expr::Bool(false)),
                ("zepp-export", Expr::Bool(false)),
                ("notification-out", Expr::Bool(false)),
                ("mini-program-bridge", Expr::Bool(false)),
                ("wifi-lan", Expr::Bool(false)),
                ("mic-relay", Expr::Bool(false)),
            ]),
        ),
        ("rate", RateClass::watch().to_expr()),
        ("firmware", Expr::Nil),
        (
            "notes",
            build::list(vec![build::text(
                "verified flags change only with hardware bring-up ledger evidence",
            )]),
        ),
    ])
}
