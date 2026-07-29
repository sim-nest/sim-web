#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! A reversible, evidence-preserving surface over solved interference studies.
//!
//! The crate is deliberately a thin leaf. It decodes the published
//! `interference/Study` Citizen, asks `sim-lib-interference-solve` to perform
//! detector-aware projection, and hands the bounded scalar result to the
//! general `scene/heatmap` contract. Edits compile to the existing
//! `interference/project` or `interference/solve` forms; no propagation,
//! resampling equation, Tensor store, or device API lives here.

mod intent;
mod scene;
mod surface;

pub use surface::{
    INTERFERENCE_PROJECT_CAPABILITY, INTERFERENCE_SOLVE_CAPABILITY, INTERFERENCE_SURFACE_CODEC_ID,
    InterferenceSurfaceCodec, MAX_ANIMATION_FRAMES, register_interference_surface,
    surface_interference_codec_symbol,
};

#[cfg(test)]
mod tests;
