//! Lossless canonical text for the scene-data subset of `Expr`.
//!
//! Scenes are pure data, so `codec:scene` serializes them through the
//! codec-neutral portable value form that lives in `sim-codec` (shared with
//! `codec:intent` and any other domain codec that must round-trip data without
//! borrowing a general codec's grammar). This module re-exports that form under
//! the names `codec:scene` uses.

pub use sim_codec::{decode_portable as decode, encode_portable as encode};
