//! Browser-local wasm placement descriptors and headless execution harness.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_stream_core::{
    ClockDomain, LatencyClass, PlacedFragment, StreamEdge, StreamEnvelope, StreamMedia,
};

use crate::transport::TransportKind;

/// Stable site name for a browser-local wasm placement.
pub fn browser_wasm_site_symbol() -> Symbol {
    Symbol::qualified("stream/site", "browser-wasm")
}

/// Stable entrypoint name for the browser wasm stream engine.
pub fn browser_wasm_engine_entry_symbol() -> Symbol {
    Symbol::qualified("stream/wasm-entry", "browser-wasm-engine")
}

/// Stable entrypoint name for the browser AudioWorklet bridge.
pub fn browser_audio_worklet_entry_symbol() -> Symbol {
    Symbol::qualified("stream/wasm-entry", "browser-audio-worklet")
}

/// Diagnostic emitted when a server-only node is offered to browser placement.
pub fn browser_server_only_refusal_diagnostic() -> Symbol {
    Symbol::qualified("stream/browser-diagnostic", "server-only-refused")
}

/// Lane a browser-local placement carries data on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserBridgeLane {
    /// UI-facing lane.
    Ui,
    /// Preview lane.
    Preview,
    /// Trace lane.
    Trace,
}

impl BrowserBridgeLane {
    /// Returns the stable symbol naming this lane.
    pub fn symbol(self) -> Symbol {
        match self {
            Self::Ui => Symbol::qualified("stream/browser-bridge", "ui"),
            Self::Preview => Symbol::qualified("stream/browser-bridge", "preview"),
            Self::Trace => Symbol::qualified("stream/browser-bridge", "trace"),
        }
    }
}

/// Wasm entrypoint symbols for a browser-local stream engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserWasmEntryPoints {
    engine: Symbol,
    audio_worklet: Symbol,
}

impl BrowserWasmEntryPoints {
    /// Returns the default browser entrypoints (engine and AudioWorklet).
    pub fn browser_defaults() -> Self {
        Self {
            engine: browser_wasm_engine_entry_symbol(),
            audio_worklet: browser_audio_worklet_entry_symbol(),
        }
    }

    /// Returns the stream engine entrypoint symbol.
    pub fn engine(&self) -> &Symbol {
        &self.engine
    }

    /// Returns the AudioWorklet bridge entrypoint symbol.
    pub fn audio_worklet(&self) -> &Symbol {
        &self.audio_worklet
    }
}

/// A browser-local wasm stream engine and its capabilities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserWasmEngine {
    id: Symbol,
    entry_points: BrowserWasmEntryPoints,
    audio_worklet_capable: bool,
}

impl BrowserWasmEngine {
    /// Builds a browser-local engine with default entrypoints.
    pub fn browser_local(id: Symbol) -> Self {
        Self {
            id,
            entry_points: BrowserWasmEntryPoints::browser_defaults(),
            audio_worklet_capable: true,
        }
    }

    /// Returns the engine id.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the engine entrypoint symbols.
    pub fn entry_points(&self) -> &BrowserWasmEntryPoints {
        &self.entry_points
    }

    /// Returns whether the engine can drive an AudioWorklet.
    pub fn audio_worklet_capable(&self) -> bool {
        self.audio_worklet_capable
    }

    /// Returns the transport kind used to reach this engine.
    pub fn transport_kind(&self) -> TransportKind {
        TransportKind::Wasm
    }

    /// Returns whether this engine tunnels audio through the server.
    pub fn uses_server_audio_tunnel(&self) -> bool {
        false
    }
}

/// A request to place a stream fragment on a browser-local engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserPlacementRequest {
    fragment: PlacedFragment,
    engine: BrowserWasmEngine,
    server_only: bool,
}

impl BrowserPlacementRequest {
    /// Builds a placement request for a fragment and target engine.
    pub fn new(fragment: PlacedFragment, engine: BrowserWasmEngine) -> Self {
        Self {
            fragment,
            engine,
            server_only: false,
        }
    }

    /// Marks the fragment as server-only, returning the updated request.
    pub fn with_server_only(mut self, server_only: bool) -> Self {
        self.server_only = server_only;
        self
    }

    /// Runs the placement headlessly, returning a report or a refusal error.
    ///
    /// # Errors
    ///
    /// Returns an error when the fragment is server-only and therefore cannot
    /// run in browser-wasm placement.
    pub fn run_headless(&self) -> Result<BrowserPlacementReport> {
        if self.server_only {
            let diagnostic = browser_server_only_refusal_diagnostic();
            return Err(Error::Eval(format!(
                "{}: server-only nodes cannot run in browser-wasm placement",
                diagnostic.as_qualified_str()
            )));
        }

        Ok(BrowserPlacementReport {
            fragment_id: self.fragment.id().clone(),
            site: browser_wasm_site_symbol(),
            engine: self.engine.clone(),
            lanes: carried_lanes(&self.fragment),
            output_envelopes: self.fragment.output_envelopes(),
            diagnostics: Vec::new(),
        })
    }
}

/// The result of running a browser-local placement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserPlacementReport {
    fragment_id: Symbol,
    site: Symbol,
    engine: BrowserWasmEngine,
    lanes: Vec<BrowserBridgeLane>,
    output_envelopes: Vec<StreamEnvelope>,
    diagnostics: Vec<Symbol>,
}

impl BrowserPlacementReport {
    /// Returns the placed fragment id.
    pub fn fragment_id(&self) -> &Symbol {
        &self.fragment_id
    }

    /// Returns the site the fragment was placed on.
    pub fn site(&self) -> &Symbol {
        &self.site
    }

    /// Returns the engine that ran the fragment.
    pub fn engine(&self) -> &BrowserWasmEngine {
        &self.engine
    }

    /// Returns the lanes the placement carries.
    pub fn lanes(&self) -> &[BrowserBridgeLane] {
        &self.lanes
    }

    /// Returns the output stream envelopes produced by the placement.
    pub fn output_envelopes(&self) -> &[StreamEnvelope] {
        &self.output_envelopes
    }

    /// Returns the diagnostics emitted during placement.
    pub fn diagnostics(&self) -> &[Symbol] {
        &self.diagnostics
    }

    /// Encodes the report as an `Expr` map.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("fragment")),
                Expr::Symbol(self.fragment_id.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("site")),
                Expr::Symbol(self.site.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("transport")),
                Expr::Symbol(Symbol::qualified("stream/transport", "wasm")),
            ),
            (
                Expr::Symbol(Symbol::new("engine")),
                Expr::Symbol(self.engine.id().clone()),
            ),
            (
                Expr::Symbol(Symbol::new("entry-points")),
                Expr::List(vec![
                    Expr::Symbol(self.engine.entry_points().engine().clone()),
                    Expr::Symbol(self.engine.entry_points().audio_worklet().clone()),
                ]),
            ),
            (
                Expr::Symbol(Symbol::new("lanes")),
                Expr::List(
                    self.lanes
                        .iter()
                        .map(|lane| Expr::Symbol(lane.symbol()))
                        .collect(),
                ),
            ),
            (
                Expr::Symbol(Symbol::new("diagnostics")),
                Expr::List(self.diagnostics.iter().cloned().map(Expr::Symbol).collect()),
            ),
        ])
    }
}

fn carried_lanes(fragment: &PlacedFragment) -> Vec<BrowserBridgeLane> {
    let mut lanes = Vec::new();
    for lane in fragment
        .input_edges()
        .iter()
        .chain(fragment.output_edges())
        .filter_map(lane_for_edge)
    {
        if !lanes.contains(&lane) {
            lanes.push(lane);
        }
    }
    lanes
}

fn lane_for_edge(edge: &StreamEdge) -> Option<BrowserBridgeLane> {
    let rate = edge.rate_contract();
    if rate.clock_domain() == ClockDomain::TraceStep {
        return Some(BrowserBridgeLane::Trace);
    }
    if edge.metadata().media() == StreamMedia::Pcm
        && rate.latency_class() == LatencyClass::BufferedPreview
    {
        return Some(BrowserBridgeLane::Preview);
    }
    if rate.clock_domain() == ClockDomain::BrowserFrame
        || edge.metadata().media() == StreamMedia::Data
    {
        return Some(BrowserBridgeLane::Ui);
    }
    None
}
