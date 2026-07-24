//! Network transports (wasm, local server, remote server).
//!
//! These share the [`Transport`] contract with the fixture so they are
//! interchangeable behind the session bridge. Each connects its runtime through
//! `realize`/`EvalFabric` (HTTP bootstrap plus a WebSocket live channel for the
//! server transports, the in-process fabric for wasm). Disconnected transports
//! fail closed, so the session degrades to a visible state rather than a crash.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use sim_kernel::{Consistency, Cx, Error, EvalMode, EvalReply, EvalRequest, Expr, Result, Symbol};
use sim_lib_server::{
    EvalSite, FrameEnvelope, ServerAddress, ServerFrame, connect_transport_site,
    eval_reply_from_frame, server_frame_from_request,
};
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, PushResult, StreamDirection, StreamEnvelope,
    StreamInspectorSnapshot, StreamItem, StreamMedia, StreamMetadata, StreamStats,
    TransportProfile, stream_inspector_route_local_symbol,
};
use sim_lib_stream_fabric::{StreamControl, stream_control_frame_from_control};
use sim_lib_view::Operation;

use crate::transport::{
    BrowserStreamStatus, ChangeEvent, SessionStatus, StreamInspectorRecord, Transport,
    TransportKind,
};

/// A network-backed transport that connects a runtime over `realize`.
pub struct RemoteTransport {
    kind: TransportKind,
    status: SessionStatus,
    endpoint: String,
    address: ServerAddress,
    offered_codecs: Vec<Symbol>,
    codec: Option<Symbol>,
    site: Option<Arc<dyn EvalSite>>,
    next_msg_id: u64,
    in_flight: BTreeSet<u64>,
    max_in_flight: usize,
    timeout: Option<Duration>,
}

impl RemoteTransport {
    /// A wasm transport targeting an in-browser runtime.
    pub fn wasm() -> Self {
        Self::new(
            TransportKind::Wasm,
            "wasm:local",
            ServerAddress::Wasm {
                region: "local".to_owned(),
            },
        )
    }

    /// A local-server transport (HTTP bootstrap + WebSocket live).
    pub fn local_server(endpoint: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        Self::new(
            TransportKind::LocalServer,
            endpoint.clone(),
            server_address_from_endpoint(&endpoint, true),
        )
    }

    /// A remote-server transport (HTTP bootstrap + WebSocket live).
    pub fn remote_server(endpoint: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        Self::new(
            TransportKind::RemoteServer,
            endpoint.clone(),
            server_address_from_endpoint(&endpoint, false),
        )
    }

    /// A local-server transport targeting an explicit server address.
    pub fn local_server_address(endpoint: impl Into<String>, address: ServerAddress) -> Self {
        Self::new(TransportKind::LocalServer, endpoint, address)
    }

    /// A remote-server transport targeting an explicit server address.
    pub fn remote_server_address(endpoint: impl Into<String>, address: ServerAddress) -> Self {
        Self::new(TransportKind::RemoteServer, endpoint, address)
    }

    fn new(kind: TransportKind, endpoint: impl Into<String>, address: ServerAddress) -> Self {
        Self {
            kind,
            status: SessionStatus::Disconnected,
            endpoint: endpoint.into(),
            address,
            offered_codecs: vec![Symbol::qualified("codec", "binary")],
            codec: None,
            site: None,
            next_msg_id: 1,
            in_flight: BTreeSet::new(),
            max_in_flight: 8,
            timeout: None,
        }
    }

    /// The configured endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// The server address this transport connects to.
    pub fn address(&self) -> &ServerAddress {
        &self.address
    }

    /// Override the codec offer set used during negotiation.
    pub fn with_offered_codecs(mut self, offered_codecs: Vec<Symbol>) -> Self {
        self.offered_codecs = offered_codecs;
        self
    }

    /// Override the maximum number of correlated requests allowed in flight.
    pub fn with_max_in_flight(mut self, max_in_flight: usize) -> Self {
        self.max_in_flight = max_in_flight.max(1);
        self
    }

    /// Override the request timeout used for server answers.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Request a remote connection and negotiate the server codec.
    pub fn connect(&mut self, cx: &mut Cx) -> Result<()> {
        self.status = SessionStatus::Connecting;
        match connect_transport_site(cx, self.address.clone(), self.offered_codecs.clone()) {
            Ok((site, codec)) => {
                self.site = Some(site);
                self.codec = Some(codec);
                self.in_flight.clear();
                self.status = SessionStatus::Connected;
                Ok(())
            }
            Err(error) => {
                self.site = None;
                self.codec = None;
                self.status = SessionStatus::Disconnected;
                Err(error)
            }
        }
    }

    /// Mark the remote channel disconnected.
    pub fn disconnect(&mut self) {
        self.site = None;
        self.codec = None;
        self.in_flight.clear();
        self.status = SessionStatus::Disconnected;
    }

    /// Mark the remote channel reconnecting.
    pub fn begin_reconnect(&mut self) {
        self.site = None;
        self.codec = None;
        self.in_flight.clear();
        self.status = SessionStatus::Reconnecting;
    }

    /// Close the remote channel deliberately.
    pub fn close(&mut self, cx: &mut Cx) -> Result<()> {
        if let Some(site) = self.site.take() {
            site.close_connection(cx)?;
        }
        self.codec = None;
        self.in_flight.clear();
        self.status = SessionStatus::Closed;
        Ok(())
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
            "{:?} transport to {} is {:?}; no traffic can flow",
            self.kind, self.endpoint, self.status
        ))
    }

    fn request(&mut self, cx: &mut Cx, expr: Expr) -> Result<Expr> {
        if !self.status.is_live() {
            return Err(self.not_connected());
        }
        if self.in_flight.len() >= self.max_in_flight {
            return Err(Error::HostError(format!(
                "{:?} transport to {} has {} in-flight requests (limit {})",
                self.kind,
                self.endpoint,
                self.in_flight.len(),
                self.max_in_flight
            )));
        }
        let Some(site) = self.site.clone() else {
            return Err(self.not_connected());
        };
        let Some(codec) = self.codec.clone() else {
            return Err(self.not_connected());
        };

        let msg_id = self.next_msg_id;
        self.next_msg_id = self.next_msg_id.saturating_add(1);
        self.in_flight.insert(msg_id);

        let mut frame = server_frame_from_request(cx, &codec, web_session_request(expr))?;
        frame.msg_id = Some(msg_id);
        frame.envelope.reply_codec_hint = Some(codec.clone());
        let reply = site.answer_with_timeout(cx, frame, self.timeout);
        self.in_flight.remove(&msg_id);

        let reply = match reply {
            Ok(reply) => reply,
            Err(error) => {
                self.status = SessionStatus::Disconnected;
                return Err(error);
            }
        };
        if reply.correlate != Some(msg_id) {
            return Err(Error::HostError(format!(
                "server reply correlation {:?} did not match request {msg_id}",
                reply.correlate
            )));
        }
        let EvalReply { value, .. } = eval_reply_from_frame(cx, &reply)?;
        let expr = value.object().as_expr(cx)?;
        if let Some(message) = remote_error_message(&expr) {
            return Err(Error::HostError(message));
        }
        Ok(expr)
    }

    fn stream_unavailable(&self, stream_id: &Symbol, operation: &str) -> Error {
        Error::HostError(format!(
            "cannot {operation} stream {stream_id}: {:?} transport to {} uses server eval requests for web-session resources",
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

    fn read(&mut self, cx: &mut Cx, resource: &Symbol) -> Result<Expr> {
        self.request(cx, web_session_read(resource))
    }

    fn realize_operation(
        &mut self,
        cx: &mut Cx,
        resource: &Symbol,
        operation: &Operation,
    ) -> Result<Expr> {
        self.request(cx, web_session_realize(resource, operation))
    }

    fn commit_operation(
        &mut self,
        cx: &mut Cx,
        resource: &Symbol,
        operation: &Operation,
        expected_current: Option<&Expr>,
    ) -> Result<Expr> {
        self.request(
            cx,
            web_session_commit(resource, operation, expected_current),
        )
    }

    fn drain_events(&mut self, cx: &mut Cx) -> Result<Vec<ChangeEvent>> {
        parse_changes(self.request(cx, web_session_changes())?)
    }

    fn stream_subscribe(
        &mut self,
        _cx: &mut Cx,
        stream_id: &Symbol,
    ) -> Result<StreamInspectorRecord> {
        Err(self.stream_unavailable(stream_id, "subscribe to"))
    }

    fn stream_read(
        &mut self,
        _cx: &mut Cx,
        stream_id: &Symbol,
        _limit: usize,
    ) -> Result<Vec<StreamItem>> {
        Err(self.stream_unavailable(stream_id, "read"))
    }

    fn stream_push(
        &mut self,
        _cx: &mut Cx,
        stream_id: &Symbol,
        _envelope: StreamEnvelope,
    ) -> Result<PushResult> {
        Err(self.stream_unavailable(stream_id, "push"))
    }

    fn stream_cancel(&mut self, _cx: &mut Cx, stream_id: &Symbol) -> Result<()> {
        Err(self.stream_unavailable(stream_id, "cancel"))
    }

    fn stream_stats(&mut self, _cx: &mut Cx, stream_id: &Symbol) -> Result<StreamStats> {
        Err(self.stream_unavailable(stream_id, "inspect stats for"))
    }

    fn stream_inspector(
        &mut self,
        _cx: &mut Cx,
        stream_id: &Symbol,
    ) -> Result<StreamInspectorRecord> {
        let status = match self.status {
            SessionStatus::Disconnected => BrowserStreamStatus::Disconnected,
            SessionStatus::Reconnecting => BrowserStreamStatus::Reconnecting,
            SessionStatus::Closed => BrowserStreamStatus::Cancelled,
            SessionStatus::Connected => BrowserStreamStatus::Disconnected,
            SessionStatus::Connecting => BrowserStreamStatus::Disconnected,
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

fn server_address_from_endpoint(endpoint: &str, local: bool) -> ServerAddress {
    if let Some(region) = endpoint.strip_prefix("wasm:") {
        return ServerAddress::Wasm {
            region: region.to_owned(),
        };
    }
    if let Some(thread) = endpoint
        .strip_prefix("in-process:")
        .and_then(|value| value.parse::<u64>().ok())
    {
        return ServerAddress::InProcess { thread };
    }
    if endpoint.starts_with("ws://") || endpoint.starts_with("wss://") {
        return ServerAddress::Ws {
            url: endpoint.to_owned(),
        };
    }
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return ServerAddress::Http {
            url: endpoint.to_owned(),
        };
    }
    if local {
        ServerAddress::InProcess { thread: 0 }
    } else {
        ServerAddress::Http {
            url: endpoint.to_owned(),
        }
    }
}

fn web_session_request(expr: Expr) -> EvalRequest {
    EvalRequest {
        expr,
        result_shape: None,
        required_capabilities: Vec::new(),
        deadline: None,
        consistency: Consistency::LocalFirst,
        mode: EvalMode::Eval,
        answer_limit: None,
        stream_buffer: None,
        stream: false,
        trace: false,
    }
}

fn web_session_read(resource: &Symbol) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("op")),
            Expr::Symbol(Symbol::qualified("web-session", "read")),
        ),
        (
            Expr::Symbol(Symbol::new("resource")),
            Expr::Symbol(resource.clone()),
        ),
    ])
}

fn web_session_realize(resource: &Symbol, operation: &Operation) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("op")),
            Expr::Symbol(Symbol::qualified("web-session", "realize")),
        ),
        (
            Expr::Symbol(Symbol::new("resource")),
            Expr::Symbol(resource.clone()),
        ),
        (
            Expr::Symbol(Symbol::new("operation")),
            operation.form.clone(),
        ),
    ])
}

fn web_session_commit(
    resource: &Symbol,
    operation: &Operation,
    expected_current: Option<&Expr>,
) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("op")),
            Expr::Symbol(Symbol::qualified("web-session", "commit")),
        ),
        (
            Expr::Symbol(Symbol::new("resource")),
            Expr::Symbol(resource.clone()),
        ),
        (
            Expr::Symbol(Symbol::new("operation")),
            operation.form.clone(),
        ),
        (
            Expr::Symbol(Symbol::new("expected-current")),
            expected_current.cloned().unwrap_or(Expr::Nil),
        ),
    ])
}

fn web_session_changes() -> Expr {
    Expr::Map(vec![(
        Expr::Symbol(Symbol::new("op")),
        Expr::Symbol(Symbol::qualified("web-session", "changes")),
    )])
}

fn parse_changes(expr: Expr) -> Result<Vec<ChangeEvent>> {
    let Expr::List(items) = expr else {
        return Err(Error::TypeMismatch {
            expected: "change event list",
            found: "non-list",
        });
    };
    items
        .into_iter()
        .map(|item| match item {
            Expr::Symbol(resource) => Ok(ChangeEvent { resource }),
            Expr::Map(entries) => entries
                .into_iter()
                .find_map(|(key, value)| {
                    let is_resource =
                        matches!(key, Expr::Symbol(symbol) if symbol.name.as_ref() == "resource");
                    match value {
                        Expr::Symbol(resource) if is_resource => Some(Ok(ChangeEvent { resource })),
                        _ => None,
                    }
                })
                .unwrap_or_else(|| {
                    Err(Error::HostError(
                        "change event is missing symbol resource".to_owned(),
                    ))
                }),
            _ => Err(Error::TypeMismatch {
                expected: "change event",
                found: "non-change",
            }),
        })
        .collect()
}

fn remote_error_message(expr: &Expr) -> Option<String> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    let kind = entries.iter().find_map(|(key, value)| {
        let is_error = matches!(key, Expr::Symbol(symbol) if symbol.name.as_ref() == "error");
        match value {
            Expr::Symbol(symbol) if is_error => Some(symbol.as_qualified_str()),
            Expr::String(message) if is_error => Some(message.clone()),
            _ => None,
        }
    })?;
    let message = entries
        .iter()
        .find_map(|(key, value)| {
            let is_message =
                matches!(key, Expr::Symbol(symbol) if symbol.name.as_ref() == "message");
            match value {
                Expr::String(message) if is_message => Some(message.clone()),
                _ => None,
            }
        })
        .unwrap_or_else(|| kind.clone());
    Some(format!("{kind}: {message}"))
}
