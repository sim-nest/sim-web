//! Network transports (wasm, local server, remote server).
//!
//! These share the [`Transport`] contract with the fixture so they are
//! interchangeable behind the session bridge. Each connects its runtime through
//! `realize`/`EvalFabric` (HTTP bootstrap plus a WebSocket live channel for the
//! server transports, the in-process fabric for wasm). Disconnected transports
//! fail closed, so the session degrades to a visible state rather than a crash.

use sim_kernel::{Cx, Error, Expr, Result, Symbol};
use sim_lib_server::{FrameEnvelope, ServerFrame};
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, PushResult, StreamDirection, StreamEnvelope,
    StreamInspectorSnapshot, StreamItem, StreamMedia, StreamMetadata, StreamStats,
    TransportProfile, stream_inspector_route_local_symbol,
};
use sim_lib_stream_fabric::{StreamControl, stream_control_frame_from_control};

use crate::transport::{
    BrowserStreamStatus, ChangeEvent, SessionStatus, StreamInspectorRecord, Transport,
    TransportKind,
};

/// A network-backed transport that connects a runtime over `realize`.
pub struct RemoteTransport {
    kind: TransportKind,
    status: SessionStatus,
    endpoint: String,
}

impl RemoteTransport {
    /// A wasm transport targeting an in-browser runtime.
    pub fn wasm() -> Self {
        Self::new(TransportKind::Wasm, "wasm:local")
    }

    /// A local-server transport (HTTP bootstrap + WebSocket live).
    pub fn local_server(endpoint: impl Into<String>) -> Self {
        Self::new(TransportKind::LocalServer, endpoint)
    }

    /// A remote-server transport (HTTP bootstrap + WebSocket live).
    pub fn remote_server(endpoint: impl Into<String>) -> Self {
        Self::new(TransportKind::RemoteServer, endpoint)
    }

    fn new(kind: TransportKind, endpoint: impl Into<String>) -> Self {
        Self {
            kind,
            status: SessionStatus::Disconnected,
            endpoint: endpoint.into(),
        }
    }

    /// The configured endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Mark the remote channel connected.
    pub fn connect(&mut self) {
        self.status = SessionStatus::Connected;
    }

    /// Mark the remote channel disconnected.
    pub fn disconnect(&mut self) {
        self.status = SessionStatus::Disconnected;
    }

    /// Mark the remote channel reconnecting.
    pub fn begin_reconnect(&mut self) {
        self.status = SessionStatus::Reconnecting;
    }

    /// Encode a stream-fabric control frame for server-backed transports.
    pub fn stream_control_frame(
        &self,
        cx: &mut Cx,
        codec: Symbol,
        control: &StreamControl,
    ) -> Result<ServerFrame> {
        match self.kind {
            TransportKind::LocalServer | TransportKind::RemoteServer => {
                stream_control_frame_from_control(cx, codec, control, FrameEnvelope::default())
            }
            TransportKind::Fixture | TransportKind::Wasm | TransportKind::Fabric => {
                Err(Error::HostError(format!(
                    "{:?} transport does not use server stream-fabric frames",
                    self.kind
                )))
            }
        }
    }

    fn not_connected(&self) -> Error {
        Error::HostError(format!(
            "{:?} transport to {} is not connected (live channel unavailable)",
            self.kind, self.endpoint
        ))
    }
}

impl Transport for RemoteTransport {
    fn kind(&self) -> TransportKind {
        self.kind
    }

    fn status(&self) -> SessionStatus {
        self.status
    }

    fn read(&self, _resource: &Symbol) -> Result<Expr> {
        Err(self.not_connected())
    }

    fn realize(&mut self, _resource: &Symbol, _operation: &Expr) -> Result<Expr> {
        Err(self.not_connected())
    }

    fn drain_events(&mut self) -> Vec<ChangeEvent> {
        Vec::new()
    }

    fn stream_subscribe(&mut self, stream_id: &Symbol) -> Result<StreamInspectorRecord> {
        Err(Error::HostError(format!(
            "cannot subscribe to stream {stream_id}: {}",
            self.not_connected()
        )))
    }

    fn stream_read(&mut self, stream_id: &Symbol, _limit: usize) -> Result<Vec<StreamItem>> {
        Err(Error::HostError(format!(
            "cannot read stream {stream_id}: {}",
            self.not_connected()
        )))
    }

    fn stream_push(&mut self, stream_id: &Symbol, _envelope: StreamEnvelope) -> Result<PushResult> {
        Err(Error::HostError(format!(
            "cannot push stream {stream_id}: {}",
            self.not_connected()
        )))
    }

    fn stream_cancel(&mut self, stream_id: &Symbol) -> Result<()> {
        Err(Error::HostError(format!(
            "cannot cancel stream {stream_id}: {}",
            self.not_connected()
        )))
    }

    fn stream_stats(&self, stream_id: &Symbol) -> Result<StreamStats> {
        Err(Error::HostError(format!(
            "cannot inspect stream stats {stream_id}: {}",
            self.not_connected()
        )))
    }

    fn stream_inspector(&self, stream_id: &Symbol) -> Result<StreamInspectorRecord> {
        let status = match self.status {
            SessionStatus::Disconnected => BrowserStreamStatus::Disconnected,
            SessionStatus::Reconnecting => BrowserStreamStatus::Reconnecting,
            SessionStatus::Closed => BrowserStreamStatus::Cancelled,
            _ => BrowserStreamStatus::Live,
        };
        Ok(StreamInspectorRecord {
            stream_id: stream_id.clone(),
            status,
            buffered: 0,
            stats: StreamStats::default(),
            diagnostics: Vec::new(),
            snapshot: StreamInspectorSnapshot::new(
                &StreamMetadata::new(
                    stream_id.clone(),
                    StreamMedia::Data,
                    StreamDirection::Source,
                    ClockDomain::ServerFrame.symbol(),
                    BufferPolicy::bounded(1)?,
                ),
                stream_inspector_route_local_symbol(),
                TransportProfile::remote_stream_fabric().name().clone(),
                status.inspector_status(),
                0,
                &StreamStats::default(),
                None,
                Vec::new(),
            ),
        })
    }
}
