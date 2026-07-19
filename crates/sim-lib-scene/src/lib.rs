//! Scene value model and `codec:scene` for the SIM Web-UI.
//!
//! A Scene is a portable graphical intermediate representation that is itself a
//! SIM value: a tree of scene nodes built on kernel `Value`/`Expr` using open
//! maps with a `kind` tag, never a closed kernel enum. The browser is the only
//! thing that turns a Scene into pixels; everything upstream just produces
//! Scene values. Because a Scene is a value it round-trips through
//! `codec:scene`, can be snapshotted, diffed, golden-tested, sent over the
//! wire, or read by an agent.
//!
//! This crate provides:
//!
//! - the scene node [`kinds`] (open metadata, not a closed enum);
//! - a [`model`] of builders, accessors, and fail-closed validation;
//! - a lossless canonical [`text`] form for the scene-data subset of `Expr`;
//! - the [`codec`] `codec:scene` (a domain codec) plus scene node [`shapes`];
//! - a [`diff()`]/apply pair over scenes (scene diffs are themselves values).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod build;
mod citizen;
pub mod codec;
pub mod cookbook;
pub mod diff;
pub mod glance;
pub mod kinds;
pub mod model;
pub mod shapes;
pub mod text;

pub use build::{RESERVED_DATA_KEYS, badge, box_, data_map, stack, sym, text_node};
pub use citizen::{SceneDescriptor, scene_descriptor_class_symbol};
pub use codec::{SceneCodec, SceneCodecLib, scene_codec_symbol};
pub use cookbook::text_node_demo;
pub use diff::{apply, diff};
pub use glance::{GLANCE_KIND, GlanceAction, GlanceCard, GlanceMetric, glance_card};
pub use kinds::{SCENE_KINDS, SCENE_NAMESPACE, is_known_kind, scene_kind};
pub use model::{SceneError, map, node, node_kind, validate_scene};
pub use shapes::{scene_shape_specs, scene_shape_symbol};

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
