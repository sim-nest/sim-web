use sim_kernel::Symbol;

use crate::{
    ARRANGER_OBJECT_ROLL_DEMO_FIXTURE, PERFORMANCE_KEYBOARD_DEMO_FIXTURE, PIANO_ROLL_DEMO_FIXTURE,
    arranger_object_roll_demo_scene, arranger_object_roll_demo_view,
    performance_workbench_demo_scene, piano_roll_demo_view, player_rack_demo_view,
};

#[test]
fn performance_workbench_recipe_covers_keyboard_rack_and_roll_descriptors() {
    let scene = performance_workbench_demo_scene();
    sim_lib_scene::validate_scene(&scene).expect("workbench scene");
    assert!(sim_test_support::contains_kind(&scene, "keyboard"));
    assert!(sim_test_support::contains_kind(&scene, "player-rack"));
    assert!(sim_test_support::contains_kind(&scene, "piano-roll"));

    let rack = player_rack_demo_view();
    let roll = piano_roll_demo_view();

    assert_eq!(rack.players.len(), 3);
    assert!(rack.players.iter().any(|player| {
        player.player_kind == Symbol::qualified("music/player-kind", "dual-arpeggio")
            && player.direct_record
    }));
    assert!(rack.players.iter().any(|player| {
        player.player_kind == Symbol::qualified("music/player-kind", "note-echo") && player.frozen
    }));
    assert!(!roll.live_notes.is_empty());
    assert!(!roll.generated_notes.is_empty());
}

#[test]
fn arranger_recipe_descriptor_covers_transforms_remaps_filter_and_failures() {
    let scene = arranger_object_roll_demo_scene();
    sim_lib_scene::validate_scene(&scene).expect("arranger scene");
    assert!(sim_test_support::contains_kind(&scene, "object-roll"));

    let view = arranger_object_roll_demo_view();
    let placements = view
        .lanes
        .iter()
        .flat_map(|lane| lane.placements.iter())
        .collect::<Vec<_>>();

    assert_eq!(placements.len(), 3);
    assert!(placements.iter().any(|placement| {
        placement.stretch == "fit-to-duration"
            && placement.transpose == 12
            && placement.invert == "pitch:C4"
            && placement.retrograde
            && placement.remap_pitch == "scale:minor-pentatonic"
            && placement.filter == Symbol::qualified("music/filter", "lead-only")
    }));
    assert!(
        placements
            .iter()
            .any(|placement| { placement.nested && placement.remap_pitch == "vector:modal-axis" })
    );
    assert!(placements.iter().any(|placement| {
        placement.remap_pitch == "matrix:ps3300-map"
            && placement.target == Symbol::qualified("audio-synth/parameter", "cutoff")
    }));
    assert_eq!(view.diagnostics.len(), 4);
}

#[test]
fn web_recipe_sources_are_registered_for_generated_docs() {
    assert_eq!(PERFORMANCE_KEYBOARD_DEMO_FIXTURE, "player-chain-instrument");
    assert_eq!(PIANO_ROLL_DEMO_FIXTURE, "keyboard-rack-roll");
    assert_eq!(ARRANGER_OBJECT_ROLL_DEMO_FIXTURE, "arranger-object-roll");
    for source in [
        include_str!("../recipes/02-performance-workbench/keyboard-rack-roll/recipe.toml"),
        include_str!("../recipes/02-performance-workbench/arranger-transform-filter/recipe.toml"),
        include_str!("../recipes/02-performance-workbench/descriptor-fixtures/recipe.toml"),
    ] {
        assert!(source.contains("view"));
        assert!(source.contains("codec = \"lisp\""));
    }
}
