use sim_kernel::{Expr, Symbol};
use sim_lib_stream_core::{
    ClockDomain, LatencyClass, PcmPacket, PlacedFragment, RateContract, StreamDirection,
    StreamEnvelope, StreamMedia, StreamPacket, TransportProfile, stream_edge,
};

use crate::{
    BrowserBridgeLane, BrowserPlacementRequest, BrowserWasmEngine,
    browser_audio_worklet_entry_symbol, browser_server_only_refusal_diagnostic,
    browser_wasm_engine_entry_symbol, browser_wasm_site_symbol,
};

#[test]
fn headless_browser_wasm_runs_placed_graph_and_carries_bridge_lanes() {
    let report = BrowserPlacementRequest::new(fragment(), browser_engine())
        .run_headless()
        .unwrap();

    assert_eq!(report.site(), &browser_wasm_site_symbol());
    assert_eq!(report.engine().transport_kind(), crate::TransportKind::Wasm);
    assert!(report.engine().audio_worklet_capable());
    assert!(!report.engine().uses_server_audio_tunnel());
    assert_eq!(
        report.engine().entry_points().engine(),
        &browser_wasm_engine_entry_symbol()
    );
    assert_eq!(
        report.engine().entry_points().audio_worklet(),
        &browser_audio_worklet_entry_symbol()
    );
    assert_eq!(
        report.lanes(),
        &[
            BrowserBridgeLane::Ui,
            BrowserBridgeLane::Preview,
            BrowserBridgeLane::Trace,
        ]
    );
    assert_eq!(report.output_envelopes().len(), 3);
}

#[test]
fn server_only_node_is_refused_for_browser_wasm() {
    let err = BrowserPlacementRequest::new(fragment(), browser_engine())
        .with_server_only(true)
        .run_headless()
        .unwrap_err();

    assert!(
        format!("{err}").contains(&browser_server_only_refusal_diagnostic().as_qualified_str())
    );
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
