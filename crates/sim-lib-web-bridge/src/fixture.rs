//! The fixture transport: a deterministic in-memory runtime.
//!
//! The fixture maps the bus onto an in-memory value store, so a session can be
//! driven end to end without a server or browser. It also models connection
//! loss and reconnection so the UI's session-status handling is testable.

use std::collections::BTreeMap;

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_stream_core::{
    PushResult, StreamEnvelope, StreamInspectorSnapshot, StreamItem, StreamMetadata, StreamPacket,
    StreamStats, StreamValue, TransportProfile, stream_inspector_route_local_symbol,
};

use crate::transport::{
    BrowserStreamStatus, ChangeEvent, SessionStatus, StreamInspectorRecord, Transport,
    TransportKind,
};

/// An in-memory transport for deterministic sessions and replay.
pub struct FixtureTransport {
    store: BTreeMap<Symbol, Expr>,
    streams: BTreeMap<Symbol, FixtureStream>,
    events: Vec<ChangeEvent>,
    status: SessionStatus,
}

struct FixtureStream {
    stream: StreamValue,
    buffered: usize,
    status: BrowserStreamStatus,
    diagnostics: Vec<Symbol>,
}

impl Default for FixtureTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl FixtureTransport {
    /// A connected, empty fixture.
    pub fn new() -> Self {
        Self {
            store: BTreeMap::new(),
            streams: BTreeMap::new(),
            events: Vec::new(),
            status: SessionStatus::Connected,
        }
    }

    /// Seed a resource value (builder form).
    pub fn with(mut self, resource: Symbol, value: Expr) -> Self {
        self.store.insert(resource, value);
        self
    }

    /// Seed or replace a resource value.
    pub fn set(&mut self, resource: Symbol, value: Expr) {
        self.store.insert(resource, value);
    }

    /// Seed a deterministic finite stream.
    pub fn with_finite_stream(mut self, metadata: StreamMetadata, items: Vec<StreamItem>) -> Self {
        self.set_finite_stream(metadata, items);
        self
    }

    /// Seed a push stream.
    pub fn with_push_stream(mut self, metadata: StreamMetadata) -> Self {
        self.set_push_stream(metadata);
        self
    }

    /// Seed or replace a deterministic finite stream.
    pub fn set_finite_stream(&mut self, metadata: StreamMetadata, items: Vec<StreamItem>) {
        self.streams.insert(
            metadata.id().clone(),
            FixtureStream {
                buffered: items.len(),
                stream: StreamValue::pull(metadata, items),
                status: BrowserStreamStatus::Live,
                diagnostics: Vec::new(),
            },
        );
    }

    /// Seed or replace a push stream.
    pub fn set_push_stream(&mut self, metadata: StreamMetadata) {
        self.streams.insert(
            metadata.id().clone(),
            FixtureStream {
                stream: StreamValue::push(metadata),
                buffered: 0,
                status: BrowserStreamStatus::Live,
                diagnostics: Vec::new(),
            },
        );
    }

    /// Mark a stream as refused after a profile diagnostic.
    pub fn mark_stream_refused(&mut self, stream_id: &Symbol, diagnostic: Symbol) -> Result<()> {
        let stream = self.stream_mut(stream_id)?;
        stream.status = BrowserStreamStatus::RefusedProfile;
        stream.diagnostics.push(diagnostic);
        Ok(())
    }

    /// Simulate connection loss.
    pub fn disconnect(&mut self) {
        self.status = SessionStatus::Disconnected;
    }

    /// Simulate a reconnecting transport.
    pub fn begin_reconnect(&mut self) {
        self.status = SessionStatus::Reconnecting;
    }

    /// Simulate a restored connection.
    pub fn reconnect(&mut self) {
        self.status = SessionStatus::Connected;
    }

    fn ensure_live(&self) -> Result<()> {
        if self.status.is_live() {
            Ok(())
        } else {
            Err(Error::HostError(format!(
                "fixture session is {:?}; no traffic can flow",
                self.status
            )))
        }
    }

    fn stream_ref(&self, stream_id: &Symbol) -> Result<&FixtureStream> {
        self.streams
            .get(stream_id)
            .ok_or_else(|| Error::UnknownSymbol {
                symbol: stream_id.clone(),
            })
    }

    fn stream_mut(&mut self, stream_id: &Symbol) -> Result<&mut FixtureStream> {
        self.streams
            .get_mut(stream_id)
            .ok_or_else(|| Error::UnknownSymbol {
                symbol: stream_id.clone(),
            })
    }

    fn visible_stream_status(&self, stream: &FixtureStream) -> BrowserStreamStatus {
        match self.status {
            SessionStatus::Disconnected => BrowserStreamStatus::Disconnected,
            SessionStatus::Reconnecting => BrowserStreamStatus::Reconnecting,
            _ => stream.status,
        }
    }

    fn inspector(&self, stream_id: &Symbol) -> Result<StreamInspectorRecord> {
        let stream = self.stream_ref(stream_id)?;
        let stats = stream.stream.stats()?;
        let status = self.visible_stream_status(stream);
        let queue_depth = stream.stream.queue_depth()?;
        let observed = stats
            .accepted
            .max(stats.yielded.saturating_add(queue_depth as u64));
        let snapshot = StreamInspectorSnapshot::new(
            stream.stream.metadata(),
            stream_inspector_route_local_symbol(),
            TransportProfile::memory_local().name().clone(),
            status.inspector_status(),
            queue_depth,
            &stats,
            observed.checked_sub(1),
            stream.diagnostics.clone(),
        );
        Ok(StreamInspectorRecord {
            stream_id: stream_id.clone(),
            status,
            buffered: stream.buffered,
            stats,
            diagnostics: stream.diagnostics.clone(),
            snapshot,
        })
    }
}

impl Transport for FixtureTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::Fixture
    }

    fn status(&self) -> SessionStatus {
        self.status
    }

    fn read(&self, resource: &Symbol) -> Result<Expr> {
        self.ensure_live()?;
        self.store
            .get(resource)
            .cloned()
            .ok_or_else(|| Error::UnknownSymbol {
                symbol: resource.clone(),
            })
    }

    fn realize(&mut self, resource: &Symbol, operation: &Expr) -> Result<Expr> {
        self.ensure_live()?;
        let new_value = apply_operation(self.store.get(resource), operation)?;
        self.store.insert(resource.clone(), new_value.clone());
        self.events.push(ChangeEvent {
            resource: resource.clone(),
        });
        Ok(new_value)
    }

    fn drain_events(&mut self) -> Vec<ChangeEvent> {
        std::mem::take(&mut self.events)
    }

    fn stream_subscribe(&mut self, stream_id: &Symbol) -> Result<StreamInspectorRecord> {
        self.ensure_live()?;
        self.inspector(stream_id)
    }

    fn stream_read(&mut self, stream_id: &Symbol, limit: usize) -> Result<Vec<StreamItem>> {
        self.ensure_live()?;
        let stream = self.stream_mut(stream_id)?;
        let items = stream.stream.take_packets(limit)?;
        stream.buffered = stream.buffered.saturating_sub(items.len());
        if stream.stream.stats()?.cancelled {
            stream.status = BrowserStreamStatus::Cancelled;
        } else if stream.stream.is_done()? {
            stream.status = BrowserStreamStatus::Ended;
        }
        Ok(items)
    }

    fn stream_push(&mut self, stream_id: &Symbol, envelope: StreamEnvelope) -> Result<PushResult> {
        self.ensure_live()?;
        if envelope.stream_id() != stream_id {
            return Err(Error::HostError(format!(
                "stream push envelope id {} does not match target {}",
                envelope.stream_id(),
                stream_id
            )));
        }
        let item = StreamItem::with_ticks(envelope.packet().clone(), envelope.ticks().to_vec())?;
        let stream = self.stream_mut(stream_id)?;
        let result = stream.stream.push_packet(item)?;
        match &result {
            PushResult::Accepted => {
                stream.buffered = stream.buffered.saturating_add(1);
                stream.status = BrowserStreamStatus::Live;
            }
            PushResult::DroppedNewest(item) | PushResult::DroppedOldest(item) => {
                stream.status = BrowserStreamStatus::BufferOverflow;
                if matches!(item.packet(), StreamPacket::Diagnostic(_)) {
                    stream
                        .diagnostics
                        .push(Symbol::qualified("stream/browser", "buffer-overflow"));
                }
            }
            PushResult::Rejected(_) => {
                stream.status = BrowserStreamStatus::BufferOverflow;
            }
            PushResult::Closed(_) => {
                stream.status = BrowserStreamStatus::Cancelled;
            }
        }
        Ok(result)
    }

    fn stream_cancel(&mut self, stream_id: &Symbol) -> Result<()> {
        self.ensure_live()?;
        let stream = self.stream_mut(stream_id)?;
        stream.stream.cancel()?;
        stream.buffered = 0;
        stream.status = BrowserStreamStatus::Cancelled;
        Ok(())
    }

    fn stream_stats(&self, stream_id: &Symbol) -> Result<StreamStats> {
        self.stream_ref(stream_id)?.stream.stats()
    }

    fn stream_inspector(&self, stream_id: &Symbol) -> Result<StreamInspectorRecord> {
        self.inspector(stream_id)
    }
}

/// Interpret a checked operation against the current value. The fixture
/// understands the universal editor's `set-value` operation; unknown operations
/// fail closed.
fn apply_operation(current: Option<&Expr>, operation: &Expr) -> Result<Expr> {
    let Expr::Map(entries) = operation else {
        return Err(Error::HostError("operation is not a map".to_owned()));
    };
    let op_name = entries.iter().find_map(|(key, value)| {
        let is_op = matches!(key, Expr::Symbol(symbol) if &*symbol.name == "op");
        match value {
            Expr::Symbol(symbol) if is_op => Some(symbol.name.to_string()),
            _ => None,
        }
    });
    match op_name.as_deref() {
        Some("set-value") => entries
            .iter()
            .find_map(|(key, value)| {
                matches!(key, Expr::Symbol(symbol) if &*symbol.name == "value").then_some(value)
            })
            .cloned()
            .ok_or_else(|| Error::HostError("set-value operation is missing a 'value'".to_owned())),
        Some(other) => Err(Error::HostError(format!(
            "fixture transport cannot realize operation '{other}'"
        ))),
        None => {
            let _ = current;
            Err(Error::HostError(
                "operation is missing an 'op' tag".to_owned(),
            ))
        }
    }
}
