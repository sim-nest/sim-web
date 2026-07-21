//! Device-class projection profiles.
//!
//! VIEW_4 promotes the web from "the view surface" to one surface among many.
//! The capability presets (`watch`/`glasses`/`phone`/`desktop`, plus
//! `cli`/`tui`/`webui`) live in [`crate::surface`]; the projection that fits a
//! Scene to a surface lives in [`crate::codec::reduce_for_caps`]. This module
//! ties them together and proves, via fixtures, that one semantic Scene projects
//! DIFFERENTLY and DETERMINISTICALLY for each device class -- a glance watch sees
//! a one-line summary where a dense desktop sees the whole tree, from the same
//! input.
//!
//! # Example
//!
//! ```
//! use sim_lib_view::profiles::project_for_preset;
//!
//! let scene = sim_lib_scene::build::stack(
//!     "column",
//!     vec![
//!         sim_lib_scene::build::text_node("a"),
//!         sim_lib_scene::build::text_node("b"),
//!         sim_lib_scene::build::text_node("c"),
//!     ],
//! );
//! // The same Scene reduces hard for a glance watch, not at all for a desktop.
//! let watch = project_for_preset(&scene, "watch").unwrap();
//! let desktop = project_for_preset(&scene, "desktop").unwrap();
//! assert!(sim_lib_scene::validate_scene(&watch).is_ok());
//! assert_ne!(watch, desktop);
//! ```

use sim_kernel::Expr;

use crate::codec::reduce_for_caps;
use crate::surface;

/// The device-class presets this profile set covers, beyond `cli`/`tui`/`webui`.
pub const DEVICE_PRESETS: &[&str] = &["watch", "glasses", "phone", "desktop"];

/// Projects `scene` toward the named surface preset's capabilities.
///
/// Looks up the preset in [`crate::surface::preset`] and reduces the Scene to its
/// display density via [`crate::codec::reduce_for_caps`]. Returns `None` for an
/// unknown preset. Deterministic for a given `(scene, preset_name)`.
pub fn project_for_preset(scene: &Expr, preset_name: &str) -> Option<Expr> {
    let caps = surface::preset(preset_name)?;
    Some(reduce_for_caps(scene, &caps))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rich_scene() -> Expr {
        sim_lib_scene::build::stack(
            "column",
            vec![
                sim_lib_scene::build::text_node("one"),
                sim_lib_scene::build::text_node("two"),
                sim_lib_scene::build::text_node("three"),
                sim_lib_scene::build::text_node("four"),
                sim_lib_scene::build::text_node("five"),
            ],
        )
    }

    fn child_count(scene: &Expr) -> usize {
        let Expr::Map(entries) = scene else {
            return 0;
        };
        for (key, value) in entries {
            match (key, value) {
                (Expr::Symbol(symbol), Expr::List(items)) if &*symbol.name == "children" => {
                    return items.len();
                }
                _ => {}
            }
        }
        0
    }

    #[test]
    fn each_device_class_projects_deterministically_and_validly() {
        let scene = rich_scene();
        for preset in DEVICE_PRESETS {
            let first = project_for_preset(&scene, preset).unwrap();
            let second = project_for_preset(&scene, preset).unwrap();
            assert_eq!(first, second, "{preset} projection must be deterministic");
            assert!(sim_lib_scene::validate_scene(&first).is_ok());
        }
    }

    #[test]
    fn glance_classes_reduce_harder_than_dense() {
        let scene = rich_scene();
        // watch + glasses are glance density -> keep 1; phone is compact -> keep 3;
        // desktop is dense -> keep all 5.
        assert_eq!(
            child_count(&project_for_preset(&scene, "watch").unwrap()),
            1
        );
        assert_eq!(
            child_count(&project_for_preset(&scene, "glasses").unwrap()),
            1
        );
        assert_eq!(
            child_count(&project_for_preset(&scene, "phone").unwrap()),
            3
        );
        assert_eq!(
            child_count(&project_for_preset(&scene, "desktop").unwrap()),
            5
        );
    }

    #[test]
    fn unknown_preset_projects_to_none() {
        assert!(project_for_preset(&rich_scene(), "hologram").is_none());
    }

    #[test]
    fn device_presets_carry_distinguishing_capabilities() {
        let watch = surface::preset("watch").unwrap();
        assert!(watch.input_flag("haptic-ack"));
        assert_eq!(watch.display_density().unwrap().name.as_ref(), "glance");

        let glasses = surface::preset("glasses").unwrap();
        assert!(glasses.input_flag("button"));
        assert_eq!(glasses.display_density().unwrap().name.as_ref(), "glance");

        let halo = surface::preset("glasses-hud").unwrap();
        assert!(halo.input_flag("voice"));
        assert!(halo.input_flag("tap"));

        let phone = surface::preset("phone").unwrap();
        assert!(phone.input_flag("camera"));

        let desktop = surface::preset("desktop").unwrap();
        assert!(desktop.input_flag("file-drop"));
        assert_eq!(desktop.display_density().unwrap().name.as_ref(), "dense");
    }
}
