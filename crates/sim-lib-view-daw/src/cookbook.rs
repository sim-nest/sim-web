//! Deterministic cookbook builders for DAW view recipes.

use sim_kernel::Expr;
use sim_lib_daw_session::instrument_session_fixture;
use sim_lib_scene::stack;

use crate::{
    COMPONENT_EDITOR_MANY_PARAM_FIXTURE, SCOPE_SPECTRUM_FIXTURE, arranger_object_roll_demo_scene,
    component_editor_snapshot, daw_view, performance_workbench_demo_scene, specialized_snapshot,
};

/// Build the modeled timeline Scene used by the cookbook recipe.
pub fn timeline_descriptor_demo() -> Expr {
    let scene = daw_view(&instrument_session_fixture());
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

/// Build the modeled arranger transform/filter Scene used by the cookbook.
pub fn arranger_transform_filter_demo() -> Expr {
    let scene = arranger_object_roll_demo_scene();
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

/// Build a modeled fixture gallery Scene for descriptor fixture coverage.
pub fn descriptor_fixtures_demo() -> Expr {
    let scene = stack(
        "column",
        vec![
            component_editor_snapshot(COMPONENT_EDITOR_MANY_PARAM_FIXTURE)
                .expect("known component editor fixture"),
            specialized_snapshot(SCOPE_SPECTRUM_FIXTURE).expect("known specialized fixture"),
        ],
    );
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

/// Build the keyboard/rack/roll performance workbench Scene.
pub fn keyboard_rack_roll_demo() -> Expr {
    let scene = performance_workbench_demo_scene();
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookbook_daw_demos_are_valid_scenes() {
        for scene in [
            timeline_descriptor_demo(),
            arranger_transform_filter_demo(),
            descriptor_fixtures_demo(),
            keyboard_rack_roll_demo(),
        ] {
            sim_lib_scene::validate_scene(&scene).expect("DAW cookbook scene validates");
        }
    }
}
