use sim_kernel::{Expr, Symbol};
use sim_lib_stream_core::{
    BridgeLatency, ClockDomain, LatencyClass, PcmPacket, PlacedFragment, RateContract,
    StreamDirection, StreamEnvelope, StreamMedia, StreamPacket, TransportProfile, stream_edge,
};
use sim_lib_topology::{
    Edge, Graph, Node, PlacementNodeProfile, PortRef, SiteMap, SiteProfile, place,
};
use sim_lib_web_bridge::{
    BrowserBridgeLane, BrowserPlacementRequest, BrowserWasmEngine, browser_wasm_site_symbol,
};

use crate::placement::{
    PLACEMENT_BRIDGE_TABLE_VIEW_ID, PLACEMENT_BROWSER_DIAGNOSTICS_VIEW_ID,
    PLACEMENT_DISCONNECT_FAULT, PLACEMENT_FAULT_TIMELINE_VIEW_ID, PLACEMENT_FORCED_REFUSAL_FAULT,
    PLACEMENT_GRAPH_VIEW_ID, PLACEMENT_INSPECTOR_VIEW_ID, PLACEMENT_JITTER_SPIKE_FAULT,
    PLACEMENT_LATENCY_BUDGET_VIEW_ID, PLACEMENT_REFUSAL_TABLE_VIEW_ID,
    PLACEMENT_RUNTIME_DIAGNOSTICS_VIEW_ID, PLACEMENT_WORKER_STALL_FAULT,
    PlacementRuntimeDiagnostic, placement_fault_fixture, placement_fault_fixture_names,
    placement_inspector_view, placement_inspector_view_with_diagnostics,
};

#[test]
fn placement_inspector_renders_all_local_report() {
    let report = report_for(all_audio_site_map());
    let scene = placement_inspector_view(&report);

    sim_lib_scene::validate_scene(&scene).expect("placement inspector scene is valid");
    assert!(sim_test_support::contains_kind(&scene, "stack"));
    assert!(sim_test_support::contains_kind(&scene, "graph"));
    assert_eq!(
        field(&scene, "lens"),
        Some(sym(PLACEMENT_INSPECTOR_VIEW_ID))
    );
    assert_eq!(field(&scene, "role"), Some(sym("placement-inspector")));
    assert!(contains_role(&scene, "placement-graph"));
    assert!(contains_role(&scene, "placement-latency-budget"));
    assert!(contains_symbol(&scene, "stream/clock-domain", "block"));
    assert!(contains_symbol(&scene, "stream/latency", "block-local"));
    assert!(contains_symbol_name(&scene, PLACEMENT_GRAPH_VIEW_ID));
    assert!(contains_symbol_name(
        &scene,
        PLACEMENT_LATENCY_BUDGET_VIEW_ID
    ));
}

#[test]
fn placement_inspector_renders_offloaded_bridges_and_budget() {
    let report = report_for(offloaded_fx_site_map());
    let scene = placement_inspector_view(&report);

    sim_lib_scene::validate_scene(&scene).expect("placement inspector scene is valid");
    assert!(contains_role(&scene, "placement-bridge-table"));
    assert!(contains_symbol(
        &scene,
        "stream/bridge",
        "latency-comp-delay"
    ));
    assert!(contains_symbol(
        &scene,
        "stream/bridge-diagnostic",
        "latency-comp-delay"
    ));
    assert!(contains_symbol_name(&scene, PLACEMENT_BRIDGE_TABLE_VIEW_ID));
}

#[test]
fn placement_inspector_accepts_browser_and_runtime_diagnostics() {
    let report = report_for(browser_wasm_site_map());
    let browser = browser_report();
    let runtime = PlacementRuntimeDiagnostic::new(
        Symbol::qualified("placement/runtime", "worker"),
        Symbol::qualified("placement/runtime-diagnostic", "queue-depth"),
        2,
    );
    let scene = placement_inspector_view_with_diagnostics(&report, &[browser], &[runtime], &[]);

    sim_lib_scene::validate_scene(&scene).expect("placement inspector scene is valid");
    assert!(contains_role(&scene, "placement-browser-diagnostics"));
    assert!(contains_role(&scene, "placement-runtime-diagnostics"));
    for lane in [
        BrowserBridgeLane::Ui,
        BrowserBridgeLane::Preview,
        BrowserBridgeLane::Trace,
    ] {
        assert!(contains_exact_symbol(&scene, &lane.symbol()));
    }
    assert!(contains_exact_symbol(&scene, &browser_wasm_site_symbol()));
    assert!(contains_symbol(
        &scene,
        "placement/runtime-diagnostic",
        "queue-depth"
    ));
    assert!(contains_symbol_name(
        &scene,
        PLACEMENT_BROWSER_DIAGNOSTICS_VIEW_ID
    ));
    assert!(contains_symbol_name(
        &scene,
        PLACEMENT_RUNTIME_DIAGNOSTICS_VIEW_ID
    ));
}

#[test]
fn placement_inspector_renders_refusals_and_fault_fixtures() {
    let report = report_for(illegal_realtime_site_map());
    let faults = placement_fault_fixture_names()
        .into_iter()
        .map(|name| placement_fault_fixture(name).expect("known placement fault fixture"))
        .collect::<Vec<_>>();
    let scene = placement_inspector_view_with_diagnostics(&report, &[], &[], &faults);

    sim_lib_scene::validate_scene(&scene).expect("placement inspector scene is valid");
    assert!(contains_role(&scene, "placement-refusals"));
    assert!(contains_role(&scene, "placement-fault-timeline"));
    assert!(contains_symbol(
        &scene,
        "placement/refusal",
        "realtime-pin-violation"
    ));
    for name in [
        PLACEMENT_FORCED_REFUSAL_FAULT,
        PLACEMENT_JITTER_SPIKE_FAULT,
        PLACEMENT_WORKER_STALL_FAULT,
        PLACEMENT_DISCONNECT_FAULT,
    ] {
        assert!(contains_symbol(&scene, "placement/fault", name));
    }
    assert!(contains_symbol_name(
        &scene,
        PLACEMENT_REFUSAL_TABLE_VIEW_ID
    ));
    assert!(contains_symbol_name(
        &scene,
        PLACEMENT_FAULT_TIMELINE_VIEW_ID
    ));
}

fn report_for(site_map: SiteMap) -> sim_lib_topology::PlacementReport {
    let mut cx = test_cx();
    place(&mut cx, &placement_graph(), &site_map).expect("placement graph compiles")
}

fn test_cx() -> sim_kernel::Cx {
    use std::sync::Arc;

    sim_kernel::Cx::new(
        Arc::new(sim_kernel::NoopEvalPolicy),
        Arc::new(sim_kernel::DefaultFactory),
    )
}

fn placement_graph() -> Graph {
    let mut graph = Graph::minimal("placement-view");
    let mut fx = Node::named("fx", "call");
    fx.target = Some(Expr::Symbol(Symbol::qualified("placement", "gain")));
    graph.nodes = vec![Node::named("in", "in"), fx, Node::named("out", "out")];
    graph.edges = vec![
        Edge::new(0, PortRef::output("in"), PortRef::input("fx")),
        Edge::new(1, PortRef::output("fx"), PortRef::input("out")),
    ];
    graph
}

fn all_audio_site_map() -> SiteMap {
    SiteMap::new(SiteProfile::audio_clock("audio"))
        .with_node_profile("in", block_profile())
        .with_node_profile(
            "fx",
            block_profile().with_latency(BridgeLatency::frames(16)),
        )
        .with_node_profile("out", block_profile())
}

fn offloaded_fx_site_map() -> SiteMap {
    SiteMap::new(SiteProfile::audio_clock("audio"))
        .with_site(SiteProfile::local_worker("worker"))
        .assign_node("fx", "worker")
        .with_node_profile("in", block_profile())
        .with_node_profile(
            "fx",
            block_profile().with_latency(BridgeLatency::frames(64)),
        )
        .with_node_profile("out", block_profile())
}

fn browser_wasm_site_map() -> SiteMap {
    SiteMap::new(SiteProfile::audio_clock("audio"))
        .with_site(SiteProfile::buffered_remote("browser-wasm"))
        .assign_node("fx", "browser-wasm")
        .with_node_profile("in", block_profile())
        .with_node_profile(
            "fx",
            PlacementNodeProfile::new(
                RateContract::new(
                    ClockDomain::BrowserFrame,
                    LatencyClass::BufferedPreview,
                    None,
                ),
                false,
            )
            .with_latency(BridgeLatency::packets(2)),
        )
        .with_node_profile("out", block_profile())
}

fn illegal_realtime_site_map() -> SiteMap {
    SiteMap::new(SiteProfile::audio_clock("audio"))
        .with_site(SiteProfile::local_worker("worker"))
        .assign_node("fx", "worker")
        .with_node_profile("in", block_profile())
        .with_node_profile("fx", PlacementNodeProfile::sample_exact(Some(48_000), true))
        .with_node_profile("out", block_profile())
}

fn block_profile() -> PlacementNodeProfile {
    PlacementNodeProfile::block_local()
}

fn browser_report() -> sim_lib_web_bridge::BrowserPlacementReport {
    BrowserPlacementRequest::new(fragment(), browser_engine())
        .run_headless()
        .expect("browser placement report")
}

fn browser_engine() -> BrowserWasmEngine {
    BrowserWasmEngine::browser_local(Symbol::qualified("stream/engine", "browser-headless"))
}

fn fragment() -> PlacedFragment {
    let ui = data_edge(
        "ui",
        RateContract::new(ClockDomain::BrowserFrame, LatencyClass::Interactive, None),
        Expr::String("scene-update".to_owned()),
    );
    let preview = preview_edge();
    let trace = data_edge(
        "trace",
        RateContract::trace_step(),
        Expr::String("trace-step".to_owned()),
    );

    PlacedFragment::new(Symbol::new("browser-node"), Expr::Bool(true))
        .with_output_edge(ui)
        .with_output_edge(preview)
        .with_output_edge(trace)
}

fn data_edge(port: &str, rate: RateContract, payload: Expr) -> sim_lib_stream_core::StreamEdge {
    let edge = stream_edge(port, StreamMedia::Data, StreamDirection::Source, rate);
    let envelope = edge.result_envelope(0, payload).unwrap();
    edge.with_envelopes(vec![envelope])
}

fn preview_edge() -> sim_lib_stream_core::StreamEdge {
    let edge = stream_edge(
        "preview",
        StreamMedia::Pcm,
        StreamDirection::Source,
        RateContract::new(
            ClockDomain::BrowserFrame,
            LatencyClass::BufferedPreview,
            None,
        ),
    );
    let envelope = StreamEnvelope::new(
        edge.metadata().id().clone(),
        Symbol::qualified("stream/browser-packet", "preview-0"),
        StreamMedia::Pcm,
        StreamDirection::Source,
        0,
        Vec::new(),
        ClockDomain::BrowserFrame,
        TransportProfile::buffered_pcm_preview(),
        Vec::new(),
        StreamPacket::Pcm(PcmPacket::f32(1, 1, vec![0.25]).unwrap()),
    )
    .unwrap();
    edge.with_envelopes(vec![envelope])
}

use sim_value::build::sym;

fn field(map: &Expr, name: &str) -> Option<Expr> {
    let Expr::Map(entries) = map else { return None };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(s) if &*s.name == name).then(|| value.clone())
    })
}

fn contains_role(expr: &Expr, role: &str) -> bool {
    field(expr, "role") == Some(sym(role))
        || expr_children(expr)
            .iter()
            .any(|child| contains_role(child, role))
}

fn contains_symbol(expr: &Expr, namespace: &str, name: &str) -> bool {
    match expr {
        Expr::Symbol(symbol)
            if symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name =>
        {
            true
        }
        _ => expr_children(expr)
            .iter()
            .any(|child| contains_symbol(child, namespace, name)),
    }
}

fn contains_symbol_name(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Symbol(symbol) if symbol.name.as_ref() == name => true,
        _ => expr_children(expr)
            .iter()
            .any(|child| contains_symbol_name(child, name)),
    }
}

fn contains_exact_symbol(expr: &Expr, expected: &Symbol) -> bool {
    match expr {
        Expr::Symbol(symbol) if symbol == expected => true,
        _ => expr_children(expr)
            .iter()
            .any(|child| contains_exact_symbol(child, expected)),
    }
}

fn expr_children(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) | Expr::Block(items) => {
            items.iter().collect()
        }
        Expr::Map(entries) => entries
            .iter()
            .flat_map(|(key, value)| [key, value])
            .collect(),
        Expr::Call { operator, args } => std::iter::once(operator.as_ref()).chain(args).collect(),
        Expr::Infix { left, right, .. } => vec![left, right],
        Expr::Prefix { arg, .. } | Expr::Postfix { arg, .. } => vec![arg],
        Expr::Quote { expr, .. } => vec![expr],
        Expr::Annotated { expr, annotations } => std::iter::once(expr.as_ref())
            .chain(annotations.iter().map(|(_, value)| value))
            .collect(),
        Expr::Extension { payload, .. } => vec![payload],
        _ => Vec::new(),
    }
}
