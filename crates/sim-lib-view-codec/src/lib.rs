//! Codec-aware and Shape-aware lenses for the SIM Web-UI (WEBUI_4).
//!
//! This lens family exposes SIM's strongest differentiators directly: a
//! multi-codec lens that opens one value through several codecs at once
//! (lisp/json/binary/algol) side by side as `scene/embed`s, a round-trip probe
//! panel, and a Shape lens with matcher-tree visualization, binding view, and
//! failing-match counterexamples. Read-construct (`#(Class ...)`) is surfaced as
//! the preferred round-trip path; broad eval stays capability-gated.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod multicodec;
pub mod shape;

pub use multicodec::{
    MULTI_CODEC_LENS, ProbeResult, SYSEX_COMPARISON_LENS, multi_codec_view, roundtrip_probe,
    sysex_comparison_view,
};
pub use shape::{SHAPE_LENS, shape_view};

#[cfg(test)]
mod tests;
