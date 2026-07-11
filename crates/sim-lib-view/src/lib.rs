//! View/editor codec contracts, Shape-based lens dispatch, lens stack, and the
//! universal default lens for the SIM Web-UI (WEBUI_4).
//!
//! A view is a codec object in the encode direction (`Value -> Scene`); an
//! editor is a codec object in the decode direction
//! (`(Value, Intent) -> Draft`, then `Draft -> operation`). A lens pairs a view
//! with an optional editor. View selection is overload selection, so the
//! dispatcher reuses the kernel `Shape` matcher rather than inventing a second
//! selection ladder.
//!
//! This crate provides the lens [`contract`] (open metadata plus the view and
//! editor traits), the Shape-based [`dispatch`] machinery, the universal
//! default lens, the lens [`stack`], and experience [`mode`]s.
//!
//! # Example
//!
//! Any value opens in the universal default lens, rendered at the active mode's
//! depth (Household, Builder, Systems):
//!
//! ```
//! use sim_kernel::Expr;
//! use sim_lib_view::{Mode, universal_scene};
//!
//! let scene = universal_scene(&Expr::Nil, Mode::Builder);
//! assert!(sim_lib_scene::validate_scene(&scene).is_ok());
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod citizen;
pub mod codec;
pub mod contract;
pub mod dispatch;
pub mod embed;
pub mod mode;
pub mod palette;
pub mod profiles;
mod render;
pub mod set_lens;
pub mod stack;
pub mod surface;
pub mod universal;
pub mod universal_editor;
pub mod universal_view;

pub use citizen::{ViewLensDescriptor, view_lens_descriptor_class_symbol};
pub use codec::{PairCodec, SurfaceCodec, roundtrip_holds};
pub use contract::{Draft, Editor, Lens, LensKind, LensMeta, Operation, View};
pub use dispatch::{DispatchContext, DispatchOutcome, DispatchReason, LensRegistry};
pub use embed::embed_scene;
pub use mode::{Exposure, Mode, action_exposure, denied_scene, readonly_scene, universal_scene};
pub use palette::{
    A11y, Command, CommandKind, FocusDir, a11y_of, diagnostics_scene, filter_commands, focused_id,
    move_focus, palette_intent, palette_scene, with_a11y, with_focus,
};
pub use profiles::{DEVICE_PRESETS, project_for_preset};
pub use set_lens::{active_lens, apply_set_lens, empty_pane_lenses};
pub use stack::LensStackEntry;
pub use surface::{SurfaceCaps, SurfaceError};
pub use universal::{UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, register_universal_default};
pub use universal_editor::{EDIT_MODES, UniversalEditor, render_draft};
pub use universal_view::{UniversalView, render_value};

/// Marker id for the always-matching universal default lens (lowest quality).
pub const UNIVERSAL_DEFAULT_LENS: &str = UNIVERSAL_VIEW_ID;

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod mode_tests;
#[cfg(test)]
mod stack_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod universal_tests;
