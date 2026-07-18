//! Placement report inspector Scene builders.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};
use sim_lib_stream_core::BridgeLatency;
use sim_lib_topology::{
    DomainBridge, PlacedNode, PlacementRefusal, PlacementRefusalReason, PlacementReport,
    PortLatency,
};
use sim_lib_web_bridge::BrowserPlacementReport;
use sim_value::build::uint;

/// Stable lens id for the full placement inspector.
pub const PLACEMENT_INSPECTOR_VIEW_ID: &str = "view:placement-inspector";
/// Stable lens id for the placed graph panel.
pub const PLACEMENT_GRAPH_VIEW_ID: &str = "view:placement-graph";
/// Stable lens id for the bridge table.
pub const PLACEMENT_BRIDGE_TABLE_VIEW_ID: &str = "view:placement-bridge-table";
/// Stable lens id for the latency-budget table.
pub const PLACEMENT_LATENCY_BUDGET_VIEW_ID: &str = "view:placement-latency-budget";
/// Stable lens id for placement refusal diagnostics.
pub const PLACEMENT_REFUSAL_TABLE_VIEW_ID: &str = "view:placement-refusals";
/// Stable lens id for browser-wasm placement diagnostics.
pub const PLACEMENT_BROWSER_DIAGNOSTICS_VIEW_ID: &str = "view:placement-browser-diagnostics";
/// Stable lens id for bounded runtime diagnostics.
pub const PLACEMENT_RUNTIME_DIAGNOSTICS_VIEW_ID: &str = "view:placement-runtime-diagnostics";
/// Stable lens id for deterministic placement fault fixtures.
pub const PLACEMENT_FAULT_TIMELINE_VIEW_ID: &str = "view:placement-fault-timeline";

/// Fault fixture name for a forced placement refusal.
pub const PLACEMENT_FORCED_REFUSAL_FAULT: &str = "forced-refusal";
/// Fault fixture name for a latency jitter spike.
pub const PLACEMENT_JITTER_SPIKE_FAULT: &str = "jitter-spike";
/// Fault fixture name for a stalled worker.
pub const PLACEMENT_WORKER_STALL_FAULT: &str = "worker-stall";
/// Fault fixture name for a site disconnect.
pub const PLACEMENT_DISCONNECT_FAULT: &str = "disconnect";

/// Bounded diagnostic counter accepted from a runtime or browser bridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementRuntimeDiagnostic {
    source: Symbol,
    diagnostic: Symbol,
    count: u64,
}

impl PlacementRuntimeDiagnostic {
    /// Builds a bounded diagnostic counter for the given source and code.
    pub fn new(source: Symbol, diagnostic: Symbol, count: u64) -> Self {
        Self {
            source,
            diagnostic,
            count,
        }
    }

    /// Returns the source that reported this diagnostic.
    pub fn source(&self) -> &Symbol {
        &self.source
    }

    /// Returns the diagnostic code.
    pub fn diagnostic(&self) -> &Symbol {
        &self.diagnostic
    }

    /// Returns the accumulated count for this diagnostic.
    pub fn count(&self) -> u64 {
        self.count
    }
}

/// Deterministic placement fault fixture rendered by the inspector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementFaultFixture {
    name: Symbol,
    response: Symbol,
    diagnostics: Vec<PlacementRuntimeDiagnostic>,
}

impl PlacementFaultFixture {
    /// Builds a fault fixture with the given name, response, and diagnostics.
    pub fn new(
        name: Symbol,
        response: Symbol,
        diagnostics: Vec<PlacementRuntimeDiagnostic>,
    ) -> Self {
        Self {
            name,
            response,
            diagnostics,
        }
    }

    /// Returns the fixture name.
    pub fn name(&self) -> &Symbol {
        &self.name
    }

    /// Returns the simulated runtime response for this fault.
    pub fn response(&self) -> &Symbol {
        &self.response
    }

    /// Returns the diagnostics emitted by this fault fixture.
    pub fn diagnostics(&self) -> &[PlacementRuntimeDiagnostic] {
        &self.diagnostics
    }
}

/// Returns the names of the built-in placement fault fixtures.
pub fn placement_fault_fixture_names() -> [&'static str; 4] {
    [
        PLACEMENT_FORCED_REFUSAL_FAULT,
        PLACEMENT_JITTER_SPIKE_FAULT,
        PLACEMENT_WORKER_STALL_FAULT,
        PLACEMENT_DISCONNECT_FAULT,
    ]
}

/// Returns the named placement fault fixture, if it exists.
pub fn placement_fault_fixture(name: &str) -> Option<PlacementFaultFixture> {
    let fixture = match name {
        PLACEMENT_FORCED_REFUSAL_FAULT => fault_fixture(name, "refused", 1),
        PLACEMENT_JITTER_SPIKE_FAULT => fault_fixture(name, "jitter-buffered", 3),
        PLACEMENT_WORKER_STALL_FAULT => fault_fixture(name, "worker-restarted", 1),
        PLACEMENT_DISCONNECT_FAULT => fault_fixture(name, "site-disconnected", 1),
        _ => return None,
    };
    Some(fixture)
}

/// Renders a placement report without runtime or browser-side diagnostics.
pub fn placement_inspector_view(report: &PlacementReport) -> Expr {
    placement_inspector_view_with_diagnostics(report, &[], &[], &[])
}

/// Renders a placement report with bounded runtime, browser, and fault data.
pub fn placement_inspector_view_with_diagnostics(
    report: &PlacementReport,
    browser_reports: &[BrowserPlacementReport],
    runtime_diagnostics: &[PlacementRuntimeDiagnostic],
    faults: &[PlacementFaultFixture],
) -> Expr {
    node(
        "stack",
        vec![
            ("lens", sym(PLACEMENT_INSPECTOR_VIEW_ID)),
            ("role", sym("placement-inspector")),
            ("dir", sym("column")),
            ("accepted", Expr::Bool(report.is_accepted())),
            (
                "children",
                Expr::List(vec![
                    placement_graph_view(report),
                    bridge_table_view(&report.bridges),
                    latency_budget_view(&report.latency),
                    refusal_table_view(&report.refusals),
                    runtime_diagnostics_view(runtime_diagnostics),
                    browser_diagnostics_view(browser_reports),
                    fault_timeline_view(faults),
                ]),
            ),
        ],
    )
}

fn placement_graph_view(report: &PlacementReport) -> Expr {
    node(
        "graph",
        vec![
            ("lens", sym(PLACEMENT_GRAPH_VIEW_ID)),
            ("role", sym("placement-graph")),
            (
                "nodes",
                Expr::List(report.placed.iter().map(placed_node_view).collect()),
            ),
            (
                "edges",
                Expr::List(report.bridges.iter().map(bridge_edge_view).collect()),
            ),
        ],
    )
}

fn placed_node_view(placed: &PlacedNode) -> Expr {
    node(
        "node",
        vec![
            ("id", Expr::Symbol(placed.node.as_symbol().clone())),
            (
                "title",
                Expr::String(placed.node.as_symbol().as_qualified_str()),
            ),
            ("site", Expr::Symbol(placed.site.as_symbol().clone())),
            ("clock-domain", Expr::Symbol(placed.clock_domain.symbol())),
            ("latency-class", Expr::Symbol(placed.latency_class.symbol())),
            ("realtime-pin", Expr::Bool(placed.realtime_pin)),
            (
                "status",
                node(
                    "badge",
                    vec![
                        (
                            "status",
                            sym(if placed.realtime_pin {
                                "realtime"
                            } else {
                                "placed"
                            }),
                        ),
                        (
                            "label",
                            Expr::String(if placed.realtime_pin {
                                "realtime".to_owned()
                            } else {
                                "placed".to_owned()
                            }),
                        ),
                    ],
                ),
            ),
        ],
    )
}

fn bridge_edge_view(bridge: &DomainBridge) -> Expr {
    node(
        "edge",
        vec![
            ("id", uint(u64::from(bridge.edge.0))),
            ("from", Expr::Symbol(bridge.from.as_symbol().clone())),
            ("to", Expr::Symbol(bridge.to.as_symbol().clone())),
            (
                "from-site",
                Expr::Symbol(bridge.from_site.as_symbol().clone()),
            ),
            ("to-site", Expr::Symbol(bridge.to_site.as_symbol().clone())),
            (
                "bridge-kind",
                Expr::Symbol(bridge.descriptor.kind().symbol()),
            ),
            (
                "bridge-diagnostics",
                Expr::List(
                    bridge
                        .descriptor
                        .diagnostics()
                        .iter()
                        .cloned()
                        .map(Expr::Symbol)
                        .collect(),
                ),
            ),
            ("latency", latency_value(bridge.descriptor.latency())),
        ],
    )
}

fn bridge_table_view(bridges: &[DomainBridge]) -> Expr {
    node(
        "table",
        vec![
            ("lens", sym(PLACEMENT_BRIDGE_TABLE_VIEW_ID)),
            ("role", sym("placement-bridge-table")),
            (
                "bridges",
                Expr::List(bridges.iter().map(bridge_row).collect()),
            ),
        ],
    )
}

fn bridge_row(bridge: &DomainBridge) -> Expr {
    data_map(vec![
        ("edge", uint(u64::from(bridge.edge.0))),
        ("from", Expr::Symbol(bridge.from.as_symbol().clone())),
        ("to", Expr::Symbol(bridge.to.as_symbol().clone())),
        (
            "from-site",
            Expr::Symbol(bridge.from_site.as_symbol().clone()),
        ),
        ("to-site", Expr::Symbol(bridge.to_site.as_symbol().clone())),
        (
            "bridge-kind",
            Expr::Symbol(bridge.descriptor.kind().symbol()),
        ),
        (
            "bridge-name",
            Expr::String(bridge.descriptor.name().to_owned()),
        ),
        (
            "diagnostics",
            Expr::List(
                bridge
                    .descriptor
                    .diagnostics()
                    .iter()
                    .cloned()
                    .map(Expr::Symbol)
                    .collect(),
            ),
        ),
        ("latency", latency_value(bridge.descriptor.latency())),
    ])
}

fn latency_budget_view(latencies: &[PortLatency]) -> Expr {
    node(
        "table",
        vec![
            ("lens", sym(PLACEMENT_LATENCY_BUDGET_VIEW_ID)),
            ("role", sym("placement-latency-budget")),
            (
                "latency",
                Expr::List(latencies.iter().map(latency_row).collect()),
            ),
        ],
    )
}

fn latency_row(latency: &PortLatency) -> Expr {
    data_map(vec![
        ("node", Expr::Symbol(latency.node.as_symbol().clone())),
        ("site", Expr::Symbol(latency.site.as_symbol().clone())),
        ("latency", latency_value(latency.latency)),
        (
            "latency-class",
            Expr::Symbol(latency.latency_class.symbol()),
        ),
    ])
}

fn refusal_table_view(refusals: &[PlacementRefusal]) -> Expr {
    node(
        "table",
        vec![
            ("lens", sym(PLACEMENT_REFUSAL_TABLE_VIEW_ID)),
            ("role", sym("placement-refusals")),
            (
                "refusals",
                Expr::List(refusals.iter().map(refusal_row).collect()),
            ),
        ],
    )
}

fn refusal_row(refusal: &PlacementRefusal) -> Expr {
    data_map(vec![
        ("node", Expr::Symbol(refusal.node.as_symbol().clone())),
        ("site", Expr::Symbol(refusal.site.as_symbol().clone())),
        (
            "reason",
            Expr::Symbol(refusal_reason_symbol(&refusal.reason)),
        ),
    ])
}

fn runtime_diagnostics_view(diagnostics: &[PlacementRuntimeDiagnostic]) -> Expr {
    node(
        "table",
        vec![
            ("lens", sym(PLACEMENT_RUNTIME_DIAGNOSTICS_VIEW_ID)),
            ("role", sym("placement-runtime-diagnostics")),
            (
                "diagnostics",
                Expr::List(diagnostics.iter().map(runtime_diagnostic_row).collect()),
            ),
        ],
    )
}

fn runtime_diagnostic_row(diagnostic: &PlacementRuntimeDiagnostic) -> Expr {
    data_map(vec![
        ("source", Expr::Symbol(diagnostic.source().clone())),
        ("diagnostic", Expr::Symbol(diagnostic.diagnostic().clone())),
        ("count", uint(diagnostic.count())),
    ])
}

fn browser_diagnostics_view(reports: &[BrowserPlacementReport]) -> Expr {
    node(
        "table",
        vec![
            ("lens", sym(PLACEMENT_BROWSER_DIAGNOSTICS_VIEW_ID)),
            ("role", sym("placement-browser-diagnostics")),
            (
                "reports",
                Expr::List(reports.iter().map(browser_report_row).collect()),
            ),
        ],
    )
}

fn browser_report_row(report: &BrowserPlacementReport) -> Expr {
    data_map(vec![
        ("fragment", Expr::Symbol(report.fragment_id().clone())),
        ("site", Expr::Symbol(report.site().clone())),
        ("engine", Expr::Symbol(report.engine().id().clone())),
        (
            "lanes",
            Expr::List(
                report
                    .lanes()
                    .iter()
                    .map(|lane| Expr::Symbol(lane.symbol()))
                    .collect(),
            ),
        ),
        (
            "diagnostics",
            Expr::List(
                report
                    .diagnostics()
                    .iter()
                    .cloned()
                    .map(Expr::Symbol)
                    .collect(),
            ),
        ),
        ("outputs", uint(report.output_envelopes().len() as u64)),
    ])
}

fn fault_timeline_view(faults: &[PlacementFaultFixture]) -> Expr {
    node(
        "timeline",
        vec![
            ("lens", sym(PLACEMENT_FAULT_TIMELINE_VIEW_ID)),
            ("role", sym("placement-fault-timeline")),
            ("lane", sym("placement-faults")),
            (
                "events",
                Expr::List(
                    faults
                        .iter()
                        .enumerate()
                        .map(|(index, fault)| fault_event(index, fault))
                        .collect(),
                ),
            ),
        ],
    )
}

fn fault_event(index: usize, fault: &PlacementFaultFixture) -> Expr {
    data_map(vec![
        ("at", uint(index as u64)),
        ("fault", Expr::Symbol(fault.name().clone())),
        ("response", Expr::Symbol(fault.response().clone())),
        (
            "diagnostics",
            Expr::List(
                fault
                    .diagnostics()
                    .iter()
                    .map(runtime_diagnostic_row)
                    .collect(),
            ),
        ),
    ])
}

fn fault_fixture(name: &str, response: &str, count: u64) -> PlacementFaultFixture {
    PlacementFaultFixture::new(
        Symbol::qualified("placement/fault", name),
        Symbol::qualified("placement/fault-response", response),
        vec![PlacementRuntimeDiagnostic::new(
            Symbol::qualified("placement/fault-source", name),
            Symbol::qualified("placement/fault-diagnostic", name),
            count,
        )],
    )
}

fn latency_value(latency: BridgeLatency) -> Expr {
    data_map(vec![
        ("frames", uint(latency.frame_count())),
        ("packets", uint(u64::from(latency.packet_count()))),
    ])
}

fn refusal_reason_symbol(reason: &PlacementRefusalReason) -> Symbol {
    let name = match reason {
        PlacementRefusalReason::UnknownSite => "unknown-site",
        PlacementRefusalReason::RealtimePinViolation => "realtime-pin-violation",
        PlacementRefusalReason::UnsupportedLatencyClass => "unsupported-latency-class",
        #[allow(unreachable_patterns)]
        _ => "incomparable-clock-domain",
    };
    Symbol::qualified("placement/refusal", name)
}
