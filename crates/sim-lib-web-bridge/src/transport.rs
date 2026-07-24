//! The transport trait and session status.
//!
//! The UI never speaks a transport-specific API. It targets the Intent/Scene
//! bus, which is expressed here as the [`Transport`] trait: reading a resource
//! value, realizing a checked operation against it (the `realize_final`
//! surface), and draining change events (the `realize_events` surface). Four
//! interchangeable transports implement it -- a deterministic fixture, plus
//! in-browser wasm, local server, and remote server -- so wasm, local, remote,
//! and fixture sessions are interchangeable behind the session bridge.

use sim_kernel::{CapabilityName, Cx, Expr, Result, Symbol};
use sim_lib_stream_core::{
    PushResult, StreamEnvelope, StreamInspectorSnapshot, StreamInspectorStatus, StreamItem,
    StreamStats, stream_cancel_capability, stream_open_capability, stream_push_capability,
    stream_read_capability, stream_stats_capability,
};
use sim_lib_stream_fabric::{
    stream_control_cancel_symbol, stream_control_next_symbol, stream_control_open_symbol,
    stream_control_push_symbol, stream_control_stats_symbol,
};
use sim_lib_view::Operation;

/// The visible state of a session's connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionStatus {
    /// Establishing the connection.
    Connecting,
    /// Connected and live.
    Connected,
    /// Lost the connection; the UI surfaces this rather than crashing.
    Disconnected,
    /// Attempting to restore a lost connection.
    Reconnecting,
    /// Deliberately closed.
    Closed,
}

impl SessionStatus {
    /// Whether reads and operations can flow right now.
    pub fn is_live(self) -> bool {
        matches!(self, SessionStatus::Connected)
    }
}

/// A change notification: a resource whose value was updated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeEvent {
    /// The resource that changed.
    pub resource: Symbol,
}

/// Browser-visible stream status for inspectors and transport badges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserStreamStatus {
    /// Packets can flow.
    Live,
    /// The transport is disconnected.
    Disconnected,
    /// The transport is reconnecting.
    Reconnecting,
    /// A stream profile was refused by the bridge.
    RefusedProfile,
    /// Backpressure dropped or rejected packets.
    BufferOverflow,
    /// The stream was cancelled.
    Cancelled,
    /// A finite stream ended normally.
    Ended,
}

impl BrowserStreamStatus {
    /// Stable label for browser/UI data.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Disconnected => "disconnected",
            Self::Reconnecting => "reconnecting",
            Self::RefusedProfile => "refused-profile",
            Self::BufferOverflow => "buffer-overflow",
            Self::Cancelled => "cancelled",
            Self::Ended => "ended",
        }
    }

    /// Stable symbol for inspector data.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("stream/browser-status", self.wire_label())
    }

    /// Maps this browser status to the inspector status enum.
    pub fn inspector_status(self) -> StreamInspectorStatus {
        match self {
            Self::Live => StreamInspectorStatus::Live,
            Self::Disconnected => StreamInspectorStatus::Disconnected,
            Self::Reconnecting => StreamInspectorStatus::Reconnecting,
            Self::RefusedProfile => StreamInspectorStatus::RefusedProfile,
            Self::BufferOverflow => StreamInspectorStatus::BufferOverflow,
            Self::Cancelled => StreamInspectorStatus::Cancelled,
            Self::Ended => StreamInspectorStatus::Ended,
        }
    }
}

/// Web bridge stream operations and their corresponding fabric controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebStreamOperation {
    /// Read up to `limit` packets.
    Read,
    /// Subscribe to stream metadata and status.
    Subscribe,
    /// Push one envelope into a stream.
    Push,
    /// Cancel the stream.
    Cancel,
    /// Inspect stream statistics.
    Stats,
}

impl WebStreamOperation {
    /// Returns the stable wire label for this operation.
    pub fn wire_label(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Subscribe => "subscribe",
            Self::Push => "push",
            Self::Cancel => "cancel",
            Self::Stats => "stats",
        }
    }

    /// Returns the stable symbol naming this operation.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("stream/web", self.wire_label())
    }

    /// Returns the fabric control symbol this operation maps to.
    pub fn fabric_symbol(self) -> Symbol {
        match self {
            Self::Read => stream_control_next_symbol(),
            Self::Subscribe => stream_control_open_symbol(),
            Self::Push => stream_control_push_symbol(),
            Self::Cancel => stream_control_cancel_symbol(),
            Self::Stats => stream_control_stats_symbol(),
        }
    }

    /// Returns the capability required to invoke this operation.
    pub fn capability(self) -> CapabilityName {
        match self {
            Self::Read => stream_read_capability(),
            Self::Subscribe => stream_open_capability(),
            Self::Push => stream_push_capability(),
            Self::Cancel => stream_cancel_capability(),
            Self::Stats => stream_stats_capability(),
        }
    }
}

/// Operation names exposed by the web bridge.
pub fn web_stream_operation_symbols() -> [Symbol; 5] {
    [
        WebStreamOperation::Read.symbol(),
        WebStreamOperation::Subscribe.symbol(),
        WebStreamOperation::Push.symbol(),
        WebStreamOperation::Cancel.symbol(),
        WebStreamOperation::Stats.symbol(),
    ]
}

/// Capability names required by browser-visible stream operations.
pub fn web_stream_operation_capability_names() -> Vec<CapabilityName> {
    [
        WebStreamOperation::Read,
        WebStreamOperation::Subscribe,
        WebStreamOperation::Push,
        WebStreamOperation::Cancel,
        WebStreamOperation::Stats,
    ]
    .into_iter()
    .map(WebStreamOperation::capability)
    .collect()
}

/// Inspector data shown by browser stream tools.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamInspectorRecord {
    /// Id of the inspected stream.
    pub stream_id: Symbol,
    /// Current browser-side status of the stream.
    pub status: BrowserStreamStatus,
    /// Number of buffered packets.
    pub buffered: usize,
    /// Accumulated stream statistics.
    pub stats: StreamStats,
    /// Diagnostics reported for the stream.
    pub diagnostics: Vec<Symbol>,
    /// Inspector snapshot of the stream.
    pub snapshot: StreamInspectorSnapshot,
}

/// Which kind of runtime a transport connects to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportKind {
    /// A deterministic in-memory fixture (tests, replay).
    Fixture,
    /// An in-browser wasm runtime.
    Wasm,
    /// A local server (HTTP bootstrap + WebSocket live).
    LocalServer,
    /// A remote server (HTTP bootstrap + WebSocket live).
    RemoteServer,
    /// A kernel [`EvalFabric`](sim_kernel::EvalFabric) target: commits are
    /// delegated to `realize`, proving a session is a realize target.
    Fabric,
}

/// The location-transparent bus the UI targets. Implementors map these calls to
/// `realize`/`EvalFabric` (`realize_final` for [`Transport::realize`] and
/// `realize_events` for [`Transport::drain_events`]); the fixture maps them to
/// an in-memory store.
pub trait Transport {
    /// Which kind of runtime this transport connects to.
    fn kind(&self) -> TransportKind;

    /// The current connection status.
    fn status(&self) -> SessionStatus;

    /// Read the current value of a resource.
    fn read(&mut self, cx: &mut Cx, resource: &Symbol) -> Result<Expr>;

    /// Realize a checked operation expression against a resource.
    ///
    /// This compatibility path wraps `operation` without authority metadata.
    /// Session commits should call [`Transport::realize_operation`] so required
    /// capabilities and result shapes are preserved.
    fn realize(&mut self, cx: &mut Cx, resource: &Symbol, operation: &Expr) -> Result<Expr> {
        self.realize_operation(cx, resource, &Operation::new(operation.clone()))
    }

    /// Realize a checked operation against a resource, returning the new value
    /// (the `realize_final` surface). Implementations also record a
    /// [`ChangeEvent`] for the resource.
    fn realize_operation(
        &mut self,
        cx: &mut Cx,
        resource: &Symbol,
        operation: &Operation,
    ) -> Result<Expr> {
        self.commit_operation(cx, resource, operation, None)
    }

    /// Commit an operation, optionally requiring the resource to still match
    /// `expected_current` on the server side.
    fn commit_operation(
        &mut self,
        cx: &mut Cx,
        resource: &Symbol,
        operation: &Operation,
        expected_current: Option<&Expr>,
    ) -> Result<Expr> {
        if let Some(expected) = expected_current {
            let current = self.read(cx, resource)?;
            if &current != expected {
                return Err(sim_kernel::Error::HostError(format!(
                    "resource '{resource}' is stale; refresh before committing"
                )));
            }
        }
        self.realize_operation(cx, resource, operation)
    }

    /// Drain the pending change events (the `realize_events` surface).
    fn drain_events(&mut self, cx: &mut Cx) -> Result<Vec<ChangeEvent>>;

    /// Subscribe to a stream and return browser-visible inspector data.
    fn stream_subscribe(
        &mut self,
        cx: &mut Cx,
        stream_id: &Symbol,
    ) -> Result<StreamInspectorRecord>;

    /// Read at most `limit` packets from a stream.
    fn stream_read(
        &mut self,
        cx: &mut Cx,
        stream_id: &Symbol,
        limit: usize,
    ) -> Result<Vec<StreamItem>>;

    /// Push one stream envelope.
    fn stream_push(
        &mut self,
        cx: &mut Cx,
        stream_id: &Symbol,
        envelope: StreamEnvelope,
    ) -> Result<PushResult>;

    /// Cancel a stream.
    fn stream_cancel(&mut self, cx: &mut Cx, stream_id: &Symbol) -> Result<()>;

    /// Return current stream stats.
    fn stream_stats(&mut self, cx: &mut Cx, stream_id: &Symbol) -> Result<StreamStats>;

    /// Return browser-visible inspector data without changing stream state.
    fn stream_inspector(
        &mut self,
        cx: &mut Cx,
        stream_id: &Symbol,
    ) -> Result<StreamInspectorRecord>;
}
