//! Stream inspector Scene builders.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};
use sim_lib_stream_core::{StreamInspectorSnapshot, StreamItem, StreamPacket};
use sim_value::build::uint;

/// Stable lens id for the stream list view.
pub const STREAM_LIST_VIEW_ID: &str = "view:stream-list";
/// Stable lens id for the single-stream detail view.
pub const STREAM_DETAIL_VIEW_ID: &str = "view:stream-detail";
/// Stable lens id for the stream packet preview table.
pub const STREAM_PACKET_PREVIEW_VIEW_ID: &str = "view:stream-packet-preview";
/// Stable lens id for the stream diagnostic timeline.
pub const STREAM_DIAGNOSTIC_TIMELINE_VIEW_ID: &str = "view:stream-diagnostic-timeline";

/// Builds the stream list Scene from inspector snapshots.
pub fn stream_list_view(streams: &[StreamInspectorSnapshot]) -> Expr {
    node(
        "stack",
        vec![
            ("lens", sym(STREAM_LIST_VIEW_ID)),
            ("role", sym("stream-list")),
            ("dir", sym("column")),
            (
                "streams",
                Expr::List(streams.iter().map(stream_row).collect()),
            ),
        ],
    )
}

/// Builds the detail Scene for a single stream snapshot.
pub fn stream_detail_view(stream: &StreamInspectorSnapshot) -> Expr {
    node(
        "box",
        vec![
            ("lens", sym(STREAM_DETAIL_VIEW_ID)),
            ("role", sym("stream-detail")),
            ("id", Expr::Symbol(stream.stream_id.clone())),
            ("route", Expr::Symbol(stream.route.clone())),
            ("media", Expr::Symbol(stream.media.symbol())),
            ("profile", Expr::Symbol(stream.profile.clone())),
            ("clock", Expr::Symbol(stream.clock.clone())),
            ("status", Expr::Symbol(stream.status.symbol())),
            ("queue-depth", uint(stream.queue_depth as u64)),
            ("dropped", uint(stream.dropped_count)),
            (
                "last-sequence",
                stream.last_sequence.map(uint).unwrap_or(Expr::Nil),
            ),
        ],
    )
}

/// Builds the packet preview table Scene from stream items.
pub fn stream_packet_preview_view(items: &[StreamItem]) -> Expr {
    node(
        "table",
        vec![
            ("lens", sym(STREAM_PACKET_PREVIEW_VIEW_ID)),
            ("role", sym("stream-packet-preview")),
            (
                "packets",
                Expr::List(
                    items
                        .iter()
                        .enumerate()
                        .map(|(index, item)| {
                            data_map(vec![
                                ("index", uint(index as u64)),
                                ("media", Expr::Symbol(item.packet().media().symbol())),
                                ("packet-kind", Expr::Symbol(packet_kind(item.packet()))),
                                ("ticks", uint(item.ticks().len() as u64)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ],
    )
}

/// Builds the diagnostic timeline Scene for a stream snapshot.
pub fn stream_diagnostic_timeline_view(stream: &StreamInspectorSnapshot) -> Expr {
    node(
        "timeline",
        vec![
            ("lens", sym(STREAM_DIAGNOSTIC_TIMELINE_VIEW_ID)),
            ("role", sym("stream-diagnostic-timeline")),
            ("stream", Expr::Symbol(stream.stream_id.clone())),
            (
                "diagnostics",
                Expr::List(
                    stream
                        .recent_diagnostics
                        .iter()
                        .cloned()
                        .map(Expr::Symbol)
                        .collect(),
                ),
            ),
        ],
    )
}

fn stream_row(stream: &StreamInspectorSnapshot) -> Expr {
    data_map(vec![
        ("id", Expr::Symbol(stream.stream_id.clone())),
        ("route", Expr::Symbol(stream.route.clone())),
        ("media", Expr::Symbol(stream.media.symbol())),
        ("profile", Expr::Symbol(stream.profile.clone())),
        ("clock", Expr::Symbol(stream.clock.clone())),
        ("status", Expr::Symbol(stream.status.symbol())),
        ("queue-depth", uint(stream.queue_depth as u64)),
        ("dropped", uint(stream.dropped_count)),
    ])
}

fn packet_kind(packet: &StreamPacket) -> Symbol {
    match packet {
        StreamPacket::Pcm(_) => Symbol::qualified("stream/packet", "pcm"),
        StreamPacket::Midi(_) => Symbol::qualified("stream/packet", "midi"),
        StreamPacket::Diagnostic(diagnostic) => diagnostic.kind().clone(),
        StreamPacket::Data(data) => data.kind.clone(),
    }
}
