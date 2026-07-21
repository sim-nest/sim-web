//! Intent value model, gesture algebra, and `codec:intent`.
//!
//! An Intent is a user (or agent) gesture expressed as a SIM value and decoded
//! into a checked operation. An Intent says *what the operator wants*, in terms
//! an editor can validate against a Shape before it ever touches runtime state.
//! Intents round-trip through `codec:intent` and carry an `origin.operator`
//! (human or agent) plus a logical tick for audit.
//!
//! This crate provides:
//!
//! - the Intent [`kinds`] and their required fields (open metadata);
//! - a [`model`] of origin, builders, accessors, fail-closed validation, and
//!   target resolution against a caller-supplied predicate;
//! - the [`gesture`] algebra folding raw browser gestures into one Intent;
//! - the [`wrist`] reducer folding watch buttons, touch, tap, raise, and crown
//!   input into standard Intent values;
//! - the [`codec`] `codec:intent` plus Intent kind [`shapes`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod citizen;
pub mod codec;
pub mod cookbook;
pub mod gesture;
pub mod kinds;
pub mod model;
pub mod shapes;
pub mod wrist;

pub use citizen::{IntentDescriptor, intent_descriptor_class_symbol};
pub use codec::{IntentCodec, IntentCodecLib, intent_codec_symbol};
pub use cookbook::select_intent_demo;
pub use gesture::{
    GestureRecognizer, Hit, HitRole, PointerEvent, PointerPhase, RawGesture, intent_from_gesture,
};
pub use kinds::{INTENT_KINDS, INTENT_NAMESPACE, intent_kind, is_known_kind, required_fields};
pub use model::{
    IntentError, Operator, Origin, field, intent, intent_kind_of, origin, referenced_targets,
    resolve_targets, validate_intent,
};
pub use shapes::{intent_shape_specs, intent_shape_symbol};
pub use wrist::{WristInputCapabilities, WristInputTiming, WristIntentReducer, WristRawInput};

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;

#[cfg(test)]
mod wrist_tests;
