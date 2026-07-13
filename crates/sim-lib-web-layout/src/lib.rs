//! Workspace value, panes, tabs, docks, and layout persistence.
//!
//! The workspace is one SIM value: panes, tabs, splits, docks, floating
//! inspectors, overlays, open resources, active lens per resource, mode,
//! session ref, palette state, and history ref. Because it round-trips through
//! general codecs, a workspace can be saved, shared, versioned, diffed, and
//! restored as data. Layout is data; restoring a session is decoding a value.
//!
//! This crate provides the workspace [`value`] model, [`pane`] records, the
//! [`layout`] engine (operations over the workspace value), and a [`scene`]
//! encoder for the dock/split arrangement.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod citizen;
pub mod layout;
pub mod palette;
pub mod pane;
pub mod scene;
pub mod value;

pub use citizen::{WorkspaceDescriptor, workspace_descriptor_class_symbol};
pub use layout::{LayoutOp, apply_layout_op, layout_op_from_intent};
pub use palette::{EntryKind, Palette, PaletteEntry, card_target, open_card};
pub use pane::{new_pane, pane_dock, pane_id, pane_lens, pane_resource, rect};
pub use scene::workspace_scene;
pub use value::{WORKSPACE_CLASS, focus, mode, new_workspace, panes};

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod palette_tests;
#[cfg(test)]
mod tests;
