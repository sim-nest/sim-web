//! Session and transport bridge over `realize`/`EvalFabric` for the
//! Intent/Scene bus.
//!
//! The bridge connects the browser shell to any runtime, location-transparently,
//! by targeting `realize_events`/`realize_final` rather than a transport-specific
//! API. Four interchangeable transports sit behind one trait: in-browser wasm,
//! local server, remote server, and fixture/cassette sessions for deterministic
//! tests. Both a human (through the browser) and an agent (through the agent
//! runner) are peers on this same bus.
//!
//! This crate provides the [`transport`] contract and session status, the
//! deterministic [`fixture`] transport, the network [`remote`] transports, and
//! the [`session`] bus with per-pane subscriptions and Scene-diff streaming.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod cookbook;
pub mod fabric;
pub mod fixture;
pub mod history;
pub mod host;
pub mod placement;
pub mod remote;
pub mod session;
pub mod sync;
pub mod transport;

pub use cookbook::session_fixture_demo;
pub use fabric::{FabricTransport, operation_to_request};
pub use fixture::FixtureTransport;
pub use history::{History, SessionLog, Snapshots, annotate};
pub use host::{DesktopHost, PHONE_PANE, PhoneHost};
pub use placement::{
    BrowserBridgeLane, BrowserPlacementReport, BrowserPlacementRequest, BrowserWasmEngine,
    BrowserWasmEntryPoints, browser_audio_worklet_entry_symbol,
    browser_server_only_refusal_diagnostic, browser_wasm_engine_entry_symbol,
    browser_wasm_site_symbol,
};
pub use remote::RemoteTransport;
pub use session::{SceneUpdate, Session};
pub use sync::{Broadcast, EditRow, SurfaceHub, replay};
pub use transport::{
    BrowserStreamStatus, ChangeEvent, SessionStatus, StreamInspectorRecord, Transport,
    TransportKind, WebStreamOperation, web_stream_operation_capability_names,
    web_stream_operation_symbols,
};

/// Stable symbol for the session value carried on the bus.
pub const SESSION_CLASS: &str = "web:Session";

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod replay_tests;
#[cfg(test)]
mod tests;
