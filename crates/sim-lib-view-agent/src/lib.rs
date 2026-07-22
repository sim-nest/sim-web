//! Agent and topology composer lens plus the live monitor for SIM Web.
//!
//! The composer renders a `scene/graph` of agent nodes with typed ports, edges,
//! and nested groups, and accepts create/move/wire/unwire/delete Intents. An
//! agent on the runner can act as an operator on the same bus a human edits on,
//! so an agent-issued Intent edits the same topology the human sees live. All
//! state is backed by existing `sim-lib-topology` values; there is no second
//! model.
//!
//! [`view`] encodes a topology `Graph` to a Scene, [`editor`] applies composer
//! Intents through the topology patch engine, and [`persist`] saves and loads a
//! topology as SIM data. [`mod@monitor`] and [`replay()`] make a run inspectable.
//!
//! # Example
//!
//! A topology `Graph` opens in the composer lens as a `scene/graph` value:
//!
//! ```
//! use sim_lib_topology::Graph;
//!
//! let graph = Graph::minimal("demo");
//! let scene = sim_lib_view_agent::composer_view(&graph);
//! assert!(sim_lib_scene::validate_scene(&scene).is_ok());
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod change_capsule;
pub mod cookbook;
pub mod editor;
pub mod mission_control;
mod mission_control_fixture;
pub mod monitor;
pub mod persist;
pub mod replay;
pub mod run;
pub mod view;

pub use change_capsule::{
    CHANGE_CAPSULE_LENS, CapsuleDiff, CapsuleFairnessFacet, CapsuleLog, CapsuleReplayEvent,
    CapsuleReplaySummary, ChangeCapsuleViewState, GeneratedDocsSummary, PinPlanView,
    change_capsule_replay_frames, change_capsule_view, fake_change_capsule_state,
};
pub use cookbook::composer_demo;
pub use editor::apply_composer_intent;
pub use mission_control::{
    EvidenceEvent, ExplanationFacet, HumanGate, LeaseClaim, LeaseConflictCard,
    MISSION_CONTROL_LENS, MissionCard, MissionControlIntent, MissionControlState, ValidationState,
    evidence_from_dev_cassette, mission_control_intents, mission_control_replay_frames,
    mission_control_view,
};
pub use mission_control_fixture::fake_mission_control_state;
pub use monitor::{MONITOR_LENS, monitor_view, node_detail};
pub use persist::{composer_load, composer_save};
pub use replay::{replay, replay_final};
pub use run::{NodeStatus, RunEvent, RunState};
pub use view::{COMPOSER_LENS, composer_view};

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod change_capsule_tests;
#[cfg(test)]
mod mission_control_tests;
#[cfg(test)]
mod monitor_tests;
#[cfg(test)]
mod tests;
