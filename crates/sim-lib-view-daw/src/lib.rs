//! DAW timeline, mixer, plugin rack, player, piano-roll, and synth lenses for
//! the SIM Web-UI.
//!
//! This lens family brings the existing audio stack into the workspace with no
//! second model: `scene/timeline` arrangement, mixer strips, `scene/meter` live
//! meters, plugin-chain rack, synth parameter panels (`scene/knob`/
//! `scene/slider`), modulation matrix (`scene/matrix`), `scene/waveform`, and
//! `scene/spectrum`. Everything is driven through Intents committed via
//! `realize` and backed by existing `sim-lib-daw-session` values.
//!
//! [`daw`] is the session lens (timeline/mixer/rack), [`synth`] the synth and
//! signal lenses, and [`param`] the Intent-driven parameter and transport edits.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod arranger;
pub mod component;
pub mod daw;
pub mod dx7;
mod instrument;
pub mod keyboard;
pub mod modular;
mod modular_fixture;
pub mod param;
pub mod piano_roll;
pub mod placement;
pub mod player_rack;
pub mod poly;
pub mod ps3300;
pub mod specialized;
pub mod stream;
pub mod synth;
pub mod system55;
pub mod system700;

pub use arranger::{
    ARRANGER_OBJECT_ROLL_ACTIONS, ARRANGER_OBJECT_ROLL_DEMO_FIXTURE, ARRANGER_OBJECT_ROLL_VIEW_ID,
    ArrangerDiagnostic, ArrangerDiagnosticKind, ArrangerLane, ArrangerObjectRollPlacement,
    ArrangerObjectRollView, arranger_object_roll_demo_scene, arranger_object_roll_demo_view,
    arranger_object_roll_view,
};
pub use component::{
    COMPONENT_EDITOR_INVALID_VALUE_FIXTURE, COMPONENT_EDITOR_MANY_PARAM_FIXTURE,
    COMPONENT_EDITOR_NO_PARAM_FIXTURE, COMPONENT_EDITOR_TRACE_ONLY_FIXTURE,
    COMPONENT_EDITOR_VIEW_ID, component_editor_fixture, component_editor_fixture_names,
    component_editor_snapshot, component_editor_view,
};
pub use daw::{DAW_LENS, daw_view};
pub use dx7::{
    DX7_EDITOR_ROUTE, DX7_EDITOR_VIEW, dx7_editor_fixture_names, dx7_editor_snapshot,
    dx7_editor_view,
};
pub use keyboard::{
    PERFORMANCE_KEYBOARD_DEMO_FIXTURE, PERFORMANCE_KEYBOARD_VIEW_ID, PerformanceKeyboardBinding,
    PerformanceKeyboardState, performance_keyboard_demo_scene, performance_keyboard_view,
};
pub use modular::{
    BuilderValidation, COMPONENT_BUILDER_ACTIONS, COMPONENT_BUILDER_CORD_EDIT_FIXTURE,
    COMPONENT_BUILDER_GRAPH_EDIT_FIXTURE, COMPONENT_BUILDER_INVALID_PATCH_FIXTURE,
    COMPONENT_BUILDER_PATCH_FORMAT, COMPONENT_BUILDER_SECTION_EDIT_FIXTURE,
    COMPONENT_BUILDER_VALIDATION_CODES, COMPONENT_BUILDER_VIEW_ID, COMPONENT_CORD_VIEW_ID,
    COMPONENT_GRAPH_VIEW_ID, COMPONENT_PALETTE_VIEW_ID, component_builder_fixture_names,
    component_builder_snapshot, component_builder_view, component_cord_view, component_graph_view,
    component_palette_view, validation_display,
};
pub use param::{apply_scrub, apply_set_param};
pub use piano_roll::{
    PIANO_ROLL_DEMO_FIXTURE, PIANO_ROLL_EDIT_ACTIONS, PIANO_ROLL_VIEW_ID, PianoRollEvent,
    PianoRollLane, PianoRollLaneKind, PianoRollView, piano_roll_demo_scene, piano_roll_demo_view,
    piano_roll_view,
};
pub use placement::{
    PLACEMENT_BRIDGE_TABLE_VIEW_ID, PLACEMENT_BROWSER_DIAGNOSTICS_VIEW_ID,
    PLACEMENT_DISCONNECT_FAULT, PLACEMENT_FAULT_TIMELINE_VIEW_ID, PLACEMENT_FORCED_REFUSAL_FAULT,
    PLACEMENT_GRAPH_VIEW_ID, PLACEMENT_INSPECTOR_VIEW_ID, PLACEMENT_JITTER_SPIKE_FAULT,
    PLACEMENT_LATENCY_BUDGET_VIEW_ID, PLACEMENT_REFUSAL_TABLE_VIEW_ID,
    PLACEMENT_RUNTIME_DIAGNOSTICS_VIEW_ID, PLACEMENT_WORKER_STALL_FAULT, PlacementFaultFixture,
    PlacementRuntimeDiagnostic, placement_fault_fixture, placement_fault_fixture_names,
    placement_inspector_view, placement_inspector_view_with_diagnostics,
};
pub use player_rack::{
    PLAYER_RACK_ACTIONS, PLAYER_RACK_VIEW_ID, PlayerRackDevice, PlayerRackView,
    performance_workbench_demo_scene, player_rack_demo_scene, player_rack_demo_view,
    player_rack_view,
};
pub use poly::{POLY_SECTION_VIEW_ID, poly_section_view};
pub use ps3300::{
    PS3300_EDITOR_ROUTE, PS3300_EDITOR_VIEW, ps3300_editor_fixture_names, ps3300_editor_snapshot,
    ps3300_editor_view,
};
pub use specialized::{
    ALGORITHM_ROUTING_FIXTURE, ALGORITHM_ROUTING_VIEW_ID, ENVELOPE_CURVE_FIXTURE,
    ENVELOPE_CURVE_VIEW_ID, FILTER_RESPONSE_FIXTURE, FILTER_RESPONSE_VIEW_ID,
    FIXED_FILTER_BANK_FIXTURE, FIXED_FILTER_BANK_VIEW_ID, POLYPHONY_ACTIVITY_FIXTURE,
    POLYPHONY_ACTIVITY_VIEW_ID, RESONATOR_RESPONSE_FIXTURE, RESONATOR_RESPONSE_VIEW_ID,
    SCOPE_SPECTRUM_FIXTURE, SCOPE_SPECTRUM_VIEW_ID, SEQUENCER_STEP_GRID_FIXTURE,
    SEQUENCER_STEP_GRID_VIEW_ID, SPECIALIZED_COMPONENT_VIEW_IDS, SPECIALIZED_DECLARING_COMPONENTS,
    SYSEX_COMPARISON_FIXTURE, SYSEX_COMPARISON_VIEW_ID, SpecializedComponentView,
    WIRING_DIAGRAM_VIEW_ID, specialized_declaring_components, specialized_fixture_names,
    specialized_snapshot, specialized_view_ids,
};
pub use stream::{
    STREAM_DETAIL_VIEW_ID, STREAM_DIAGNOSTIC_TIMELINE_VIEW_ID, STREAM_LIST_VIEW_ID,
    STREAM_PACKET_PREVIEW_VIEW_ID, stream_detail_view, stream_diagnostic_timeline_view,
    stream_list_view, stream_packet_preview_view,
};
pub use synth::{SYNTH_LENS, modulation_matrix, spectrum_view, synth_panel, waveform_view};
pub use system55::{
    SYSTEM55_EDITOR_ROUTE, SYSTEM55_EDITOR_VIEW, system55_editor_fixture_names,
    system55_editor_snapshot, system55_editor_view,
};
pub use system700::{
    SYSTEM700_EDITOR_ROUTE, SYSTEM700_EDITOR_VIEW, system700_editor_fixture_names,
    system700_editor_snapshot, system700_editor_view,
};

/// Stable symbol for the DAW timeline lens.
pub const TIMELINE_LENS: &str = DAW_LENS;

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod arranger_tests;
#[cfg(test)]
mod instrument_tests;
#[cfg(test)]
mod keyboard_tests;
#[cfg(test)]
mod piano_roll_tests;
#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod recipe_tests;
#[cfg(test)]
mod tests;
