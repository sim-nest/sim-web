//! Host-side view frame facade for wasm-shaped view data.
//!
//! This crate is ordinary Rust glue for rendering values into Scene data,
//! folding raw gestures into Intents, and committing edits against an
//! in-process value. It shares the view, intent, and scene contracts used by web
//! shell adapters, but it does not provide wasm-bindgen bindings or an embedded
//! WebAssembly runtime.
//!
//! The [`host`] module provides [`BrowserHost`], a local render/edit helper that
//! renders a value to a Scene, folds raw gestures into Intents, commits edits
//! locally, and emits Scene diffs. The public type name is the stable host
//! facade entry point.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod cookbook;
pub mod host;

pub use cookbook::host_loop_demo;
pub use host::{BrowserHost, SceneUpdate};

/// Stable symbol for the view wasm host facade.
pub const WASM_VIEW_HOST: &str = "view:wasm-host";

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
