//! Deterministic cookbook builders for codec view recipes.

use sim_kernel::Expr;

use crate::{ProbeResult, sysex_comparison_view};

/// Build the multi-codec probe Scene used by the cookbook recipe.
pub fn multicodec_demo() -> Expr {
    let probe = ProbeResult::lossless("(quote sysex-demo)");
    let scene = sysex_comparison_view(
        "f0 43 10 4c 00 f7",
        &[0xf0, 0x43, 0x10, 0x4c, 0x00, 0xf7],
        "(midi/sysex [0x43 0x10 0x4c 0x00])",
        &probe,
    );
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multicodec_demo_is_a_valid_scene() {
        sim_lib_scene::validate_scene(&multicodec_demo()).expect("codec scene validates");
    }
}
