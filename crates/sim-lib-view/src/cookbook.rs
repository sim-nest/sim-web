//! Deterministic cookbook builders for view recipes.

use sim_kernel::{Expr, Symbol};
use sim_value::build;

use crate::surface::SurfaceCaps;
use crate::{Mode, UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, universal_scene};

/// Build a modeled universal view/editor descriptor for the cookbook.
pub fn universal_lens_demo() -> Expr {
    let value = build::map(vec![
        (
            "class",
            Expr::Symbol(Symbol::qualified("demo", "ViewValue")),
        ),
        ("title", build::text("Universal lens sample")),
    ]);
    let surface = SurfaceCaps::from_preset("webui", "cookbook.web")
        .expect("webui is a built-in surface preset");
    let scene = universal_scene(&value, Mode::Builder);
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    build::map(vec![
        ("surface", surface.to_expr()),
        ("view-lens", Expr::Symbol(Symbol::new(UNIVERSAL_VIEW_ID))),
        (
            "editor-lens",
            Expr::Symbol(Symbol::new(UNIVERSAL_EDITOR_ID)),
        ),
        ("scene", scene),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_value::access;

    #[test]
    fn universal_lens_demo_carries_surface_caps_and_valid_scene() {
        let demo = universal_lens_demo();
        let surface = access::field(&demo, "surface").expect("surface descriptor");
        SurfaceCaps::from_expr(surface).expect("surface caps parse");
        let scene = access::field(&demo, "scene").expect("scene");
        sim_lib_scene::validate_scene(scene).expect("scene validates");
    }
}
