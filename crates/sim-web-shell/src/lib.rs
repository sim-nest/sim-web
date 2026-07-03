//! `sim-web-shell`: the binary that serves the SIM WebUI shell.
//!
//! The crate embeds its `web/` browser assets and serves cookbook APIs through
//! the shared server/cookbook libraries. The browser shell is a thin Scene
//! painter and Intent emitter with an Atelier cache view over the generated
//! Site graph, constellation index, Retrieval Radar, and Guideline Firewall
//! reports. The shell ships no second data model and no second semantics.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod assets;
mod atelier;
mod cli;
mod live;
mod serve;

pub use assets::{Asset, asset_for};
pub use atelier::{AtelierWebResponse, AtelierWebState};
pub use cli::{AtelierCliLib, BrowseCliLib};
pub use live::{
    DEFAULT_PANE, DEFAULT_RESOURCE, LiveSession, decode_intent_body, encode_patches, encode_scene,
    error_json,
};
pub use serve::{ServeConfig, serve};

#[cfg(test)]
mod tests;
