//! Round watch glance budgets and shared adapter constructors.

use sim_kernel::{Expr, Symbol};
use sim_lib_view_device::{AckChannel, GlanceAdapter, GlanceBudget};
use sim_value::build;

/// Modeled local acknowledgement duration for a wrist tap.
pub const WATCH_GLANCE_ACK_MS: u64 = 40;

/// Abstract cells available on the 466x466 round watch face.
pub const WATCH_GLANCE_CELLS: u8 = 3;

/// Compact glyph budget for the 466x466 round watch face.
pub const WATCH_GLANCE_GLYPHS: u16 = 64;

/// Abstract cells available on the 480x480 large round watch face.
pub const WATCH_GLANCE_LARGE_CELLS: u8 = 4;

/// Compact glyph budget for the 480x480 large round watch face.
pub const WATCH_GLANCE_LARGE_GLYPHS: u16 = 96;

/// Returns the round 44mm watch-face glance budget.
pub fn watch_glance_budget() -> GlanceBudget {
    GlanceBudget {
        cells: WATCH_GLANCE_CELLS,
        glyphs: WATCH_GLANCE_GLYPHS,
        ack: AckChannel::Haptic,
    }
}

/// Returns the large round watch-face glance budget.
pub fn watch_glance_large_budget() -> GlanceBudget {
    GlanceBudget {
        cells: WATCH_GLANCE_LARGE_CELLS,
        glyphs: WATCH_GLANCE_LARGE_GLYPHS,
        ack: AckChannel::Haptic,
    }
}

/// Builds the wrist glance adapter from the shared device adapter.
pub fn watch_glance_adapter(large: bool) -> GlanceAdapter {
    let budget = if large {
        watch_glance_large_budget()
    } else {
        watch_glance_budget()
    };
    GlanceAdapter::new(budget, WATCH_GLANCE_ACK_MS)
}

/// Builds the large-face wrist glance adapter from the shared device adapter.
pub fn watch_glance_large_adapter() -> GlanceAdapter {
    watch_glance_adapter(true)
}

/// Builds the descriptor expression used by the embedded cookbook recipe.
pub fn watch_glance_budget_demo() -> Expr {
    budget_expr("amazfit-t-rex-3-pro-44", watch_glance_budget())
}

fn budget_expr(model: &str, budget: GlanceBudget) -> Expr {
    build::map(vec![
        (
            "kind",
            Expr::Symbol(Symbol::qualified("view-wrist", "glance-budget")),
        ),
        ("model", Expr::String(model.to_owned())),
        ("cells", build::uint(u64::from(budget.cells))),
        ("glyphs", build::uint(u64::from(budget.glyphs))),
        ("ack", Expr::Symbol(budget.ack.to_symbol())),
        ("ack-ms", build::uint(WATCH_GLANCE_ACK_MS)),
    ])
}
