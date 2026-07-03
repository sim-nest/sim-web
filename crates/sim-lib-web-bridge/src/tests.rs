//! End-to-end tests for the session bridge over the fixture transport.

use sim_kernel::{Expr, NumberLiteral, Symbol};
use sim_lib_intent::{Origin, intent};
use sim_lib_stream_core::{
    BufferOverflowPolicy, BufferPolicy, ClockDomain, PcmPacket, StreamDirection, StreamEnvelope,
    StreamInspectorStatus, StreamItem, StreamMedia, StreamMetadata, StreamPacket, TransportProfile,
    stream_cancel_capability, stream_open_capability, stream_push_capability,
    stream_read_capability, stream_remote_network_capability, stream_stats_capability,
};
use sim_lib_stream_fabric::{
    StreamControl, stream_control_cancel_symbol, stream_control_from_frame,
    stream_control_next_symbol, stream_control_open_symbol, stream_control_push_symbol,
    stream_control_stats_symbol,
};
use sim_lib_view::{
    LensRegistry, UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, register_universal_default,
};

use crate::fixture::FixtureTransport;
use crate::remote::RemoteTransport;
use crate::session::Session;
use crate::transport::{
    BrowserStreamStatus, SessionStatus, Transport, TransportKind, WebStreamOperation,
    web_stream_operation_capability_names, web_stream_operation_symbols,
};

use sim_kernel::testing::eager_cx as cx;

fn registry() -> LensRegistry {
    let mut registry = LensRegistry::new();
    register_universal_default(&mut registry, false);
    registry
}

use sim_value::build::keyword as sym;

fn number(value: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: sym("i64"),
        canonical: value.to_owned(),
    })
}

fn lisp_codec() -> Symbol {
    Symbol::qualified("codec", "lisp")
}

fn doc() -> Expr {
    Expr::Map(vec![
        (Expr::Symbol(sym("a")), number("1")),
        (Expr::Symbol(sym("b")), number("2")),
    ])
}

fn set_mode(name: &str) -> Expr {
    intent(
        "set-mode",
        Origin::human(1),
        vec![("mode", Expr::Symbol(sym(name)))],
    )
}

fn region_count(scene: &Expr) -> usize {
    let Expr::Map(entries) = scene else { return 0 };
    entries
        .iter()
        .find_map(|(key, value)| {
            let is_children = matches!(key, Expr::Symbol(s) if &*s.name == "children");
            match value {
                Expr::List(items) if is_children => Some(items.len()),
                _ => None,
            }
        })
        .unwrap_or(0)
}

fn edit_a_to_9() -> Expr {
    intent(
        "edit-field",
        Origin::human(1),
        vec![
            ("target", doc()),
            (
                "path",
                Expr::List(vec![Expr::Vector(vec![
                    Expr::Symbol(sym("k")),
                    Expr::Symbol(sym("a")),
                ])]),
            ),
            ("value", number("9")),
        ],
    )
}

#[test]
fn open_render_edit_commit_and_observe_the_updated_scene() {
    let mut cx = cx();
    let registry = registry();
    let transport = FixtureTransport::new().with(sym("doc"), doc());
    let mut session = Session::new(transport);

    // Open the value and render its initial Scene.
    let initial = session
        .open(
            &mut cx,
            &registry,
            sym("pane-1"),
            sym("doc"),
            sym(UNIVERSAL_VIEW_ID),
            sym(UNIVERSAL_EDITOR_ID),
        )
        .unwrap();
    sim_lib_scene::validate_scene(&initial).expect("initial scene is valid");

    // Emit an Intent; it decodes, commits through realize.
    session
        .submit_intent(&mut cx, &registry, &sym("pane-1"), &edit_a_to_9())
        .unwrap();

    // The runtime value changed.
    let value = session.transport_mut().read(&sym("doc")).unwrap();
    let Expr::Map(entries) = &value else {
        panic!("doc is a map")
    };
    let a = entries
        .iter()
        .find(|(k, _)| matches!(k, Expr::Symbol(s) if &*s.name == "a"))
        .map(|(_, v)| v);
    assert_eq!(a, Some(&number("9")));

    // Pumping re-renders the affected pane and yields a diff that reconstructs
    // the new Scene from the old one.
    let updates = session.pump(&mut cx, &registry).unwrap();
    assert_eq!(updates.len(), 1, "exactly the subscribed pane updates");
    let update = &updates[0];
    assert_eq!(update.pane, sym("pane-1"));
    assert_ne!(update.scene, initial, "the Scene changed");
    let rebuilt = sim_lib_scene::apply(&initial, &update.diff).unwrap();
    assert_eq!(rebuilt, update.scene, "the diff reconstructs the new Scene");
}

#[test]
fn connection_loss_and_reconnect_are_visible_session_state() {
    let mut cx = cx();
    let registry = registry();
    let transport = FixtureTransport::new().with(sym("doc"), doc());
    let mut session = Session::new(transport);
    assert_eq!(session.status(), SessionStatus::Connected);

    session.transport_mut().disconnect();
    assert_eq!(session.status(), SessionStatus::Disconnected);
    // While disconnected, opening fails closed rather than crashing.
    assert!(
        session
            .open(
                &mut cx,
                &registry,
                sym("pane-1"),
                sym("doc"),
                sym(UNIVERSAL_VIEW_ID),
                sym(UNIVERSAL_EDITOR_ID),
            )
            .is_err()
    );

    session.transport_mut().reconnect();
    assert_eq!(session.status(), SessionStatus::Connected);
    assert!(
        session
            .open(
                &mut cx,
                &registry,
                sym("pane-1"),
                sym("doc"),
                sym(UNIVERSAL_VIEW_ID),
                sym(UNIVERSAL_EDITOR_ID),
            )
            .is_ok()
    );
}

#[test]
fn modes_render_the_same_value_at_different_depth() {
    use sim_lib_view::Mode;

    let mut session = Session::new(FixtureTransport::new());
    assert_eq!(session.mode(), Mode::Builder);

    let value = doc();
    let builder = region_count(&session.render_universal(&value));

    // Switch to Household via intent/set-mode.
    session.set_mode(&set_mode("household")).unwrap();
    assert_eq!(session.mode(), Mode::Household);
    let household = region_count(&session.render_universal(&value));

    session.set_mode(&set_mode("systems")).unwrap();
    let systems = region_count(&session.render_universal(&value));

    assert!(
        household < builder && builder < systems,
        "depth grows with mode ({household} < {builder} < {systems})"
    );
    // The value never changed across mode switches.
    assert_eq!(value, doc());

    // A bad mode name fails closed.
    assert!(session.set_mode(&set_mode("nonsense")).is_err());
}

#[test]
fn capability_denied_actions_are_never_silently_executed() {
    use sim_kernel::CapabilityName;
    use sim_lib_view::{Exposure, Mode, action_exposure};

    let required = vec![CapabilityName::new("admin")];
    let deny = |c: &CapabilityName| c.as_str() != "admin";
    // Denied -> absent, never executed.
    assert_eq!(
        action_exposure(&required, deny, false, Mode::Systems),
        Exposure::Absent
    );
    // Granted but dangerous -> confirmation-gated, not silent.
    let grant = |_: &CapabilityName| true;
    assert_eq!(
        action_exposure(&[], grant, true, Mode::Builder),
        Exposure::ConfirmationGated
    );
}

#[test]
fn transports_are_interchangeable_behind_one_trait() {
    let fixture = FixtureTransport::new();
    assert_eq!(fixture.kind(), TransportKind::Fixture);
    assert_eq!(fixture.status(), SessionStatus::Connected);

    for transport in [
        RemoteTransport::wasm(),
        RemoteTransport::local_server("http://localhost:8787"),
        RemoteTransport::remote_server("https://sim.example"),
    ] {
        // Network transports report Disconnected and fail closed until wired.
        assert_eq!(transport.status(), SessionStatus::Disconnected);
        assert!(transport.read(&sym("doc")).is_err());
    }
}

#[test]
fn web_stream_operation_names_map_to_fabric_controls() {
    assert_eq!(
        web_stream_operation_symbols(),
        [
            Symbol::qualified("stream/web", "read"),
            Symbol::qualified("stream/web", "subscribe"),
            Symbol::qualified("stream/web", "push"),
            Symbol::qualified("stream/web", "cancel"),
            Symbol::qualified("stream/web", "stats"),
        ]
    );
    assert_eq!(
        WebStreamOperation::Read.fabric_symbol(),
        stream_control_next_symbol()
    );
    assert_eq!(
        WebStreamOperation::Subscribe.fabric_symbol(),
        stream_control_open_symbol()
    );
    assert_eq!(
        WebStreamOperation::Push.fabric_symbol(),
        stream_control_push_symbol()
    );
    assert_eq!(
        WebStreamOperation::Cancel.fabric_symbol(),
        stream_control_cancel_symbol()
    );
    assert_eq!(
        WebStreamOperation::Stats.fabric_symbol(),
        stream_control_stats_symbol()
    );
    assert_eq!(
        web_stream_operation_capability_names()
            .into_iter()
            .map(|capability| capability.as_str().to_owned())
            .collect::<Vec<_>>(),
        vec![
            "stream.read",
            "stream.open",
            "stream.push",
            "stream.cancel",
            "stream.stats",
        ]
    );
    assert_eq!(
        WebStreamOperation::Read.capability(),
        stream_read_capability()
    );
    assert_eq!(
        WebStreamOperation::Subscribe.capability(),
        stream_open_capability()
    );
    assert_eq!(
        WebStreamOperation::Push.capability(),
        stream_push_capability()
    );
    assert_eq!(
        WebStreamOperation::Cancel.capability(),
        stream_cancel_capability()
    );
    assert_eq!(
        WebStreamOperation::Stats.capability(),
        stream_stats_capability()
    );
}

#[test]
fn fixture_transport_supports_deterministic_finite_streams() {
    let metadata = pcm_metadata("stream/web-finite", 4);
    let mut transport = FixtureTransport::new()
        .with_finite_stream(metadata.clone(), vec![pcm_item(1.0), pcm_item(2.0)]);

    let inspector = transport.stream_subscribe(metadata.id()).unwrap();
    assert_eq!(inspector.stream_id, metadata.id().clone());
    assert_eq!(inspector.status, BrowserStreamStatus::Live);
    assert_eq!(inspector.buffered, 2);
    assert_eq!(inspector.snapshot.stream_id, metadata.id().clone());
    assert_eq!(inspector.snapshot.queue_depth, 2);
    assert_eq!(inspector.snapshot.last_sequence, Some(1));
    assert_eq!(inspector.snapshot.status, StreamInspectorStatus::Live);

    let first = transport.stream_read(metadata.id(), 1).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(transport.stream_stats(metadata.id()).unwrap().yielded, 1);
    assert_eq!(
        transport.stream_inspector(metadata.id()).unwrap().buffered,
        1
    );

    let rest = transport.stream_read(metadata.id(), 8).unwrap();
    assert_eq!(rest.len(), 1);
    assert_eq!(
        transport.stream_inspector(metadata.id()).unwrap().status,
        BrowserStreamStatus::Ended
    );
}

#[test]
fn fixture_stream_inspector_reports_browser_statuses() {
    let finite_metadata = pcm_metadata("stream/status-finite", 4);
    let push_metadata = pcm_metadata_with_overflow("stream/status-push", 1);
    let mut transport = FixtureTransport::new()
        .with_finite_stream(finite_metadata.clone(), vec![pcm_item(0.0)])
        .with_push_stream(push_metadata.clone());

    transport.disconnect();
    let disconnected = transport.stream_inspector(finite_metadata.id()).unwrap();
    assert_eq!(disconnected.status, BrowserStreamStatus::Disconnected);
    assert_eq!(
        disconnected.snapshot.status,
        StreamInspectorStatus::Disconnected
    );
    transport.begin_reconnect();
    assert_eq!(
        transport
            .stream_inspector(finite_metadata.id())
            .unwrap()
            .status,
        BrowserStreamStatus::Reconnecting
    );
    transport.reconnect();
    transport
        .mark_stream_refused(
            finite_metadata.id(),
            Symbol::qualified("stream/fabric", "RefusedProfile"),
        )
        .unwrap();
    let inspector = transport.stream_inspector(finite_metadata.id()).unwrap();
    assert_eq!(inspector.status, BrowserStreamStatus::RefusedProfile);
    assert_eq!(
        inspector.snapshot.status,
        StreamInspectorStatus::RefusedProfile
    );
    assert_eq!(
        inspector.diagnostics,
        vec![Symbol::qualified("stream/fabric", "RefusedProfile")]
    );

    assert!(matches!(
        transport
            .stream_push(push_metadata.id(), pcm_envelope(push_metadata.id(), 0, 1.0))
            .unwrap(),
        sim_lib_stream_core::PushResult::Accepted
    ));
    assert!(matches!(
        transport
            .stream_push(push_metadata.id(), pcm_envelope(push_metadata.id(), 1, 2.0))
            .unwrap(),
        sim_lib_stream_core::PushResult::Rejected(_)
    ));
    assert_eq!(
        transport
            .stream_inspector(push_metadata.id())
            .unwrap()
            .status,
        BrowserStreamStatus::BufferOverflow
    );
    assert_eq!(
        transport
            .stream_inspector(push_metadata.id())
            .unwrap()
            .snapshot
            .status,
        StreamInspectorStatus::BufferOverflow
    );

    transport.stream_cancel(push_metadata.id()).unwrap();
    assert_eq!(
        transport
            .stream_inspector(push_metadata.id())
            .unwrap()
            .status,
        BrowserStreamStatus::Cancelled
    );
}

#[test]
fn server_transports_encode_stream_controls_as_fabric_frames() {
    let mut cx = cx();
    let codec_id = cx.registry_mut().fresh_codec_id();
    cx.load_lib(&sim_codec_lisp::LispCodecLib::new(codec_id).unwrap())
        .unwrap();
    let transport = RemoteTransport::local_server("http://localhost:8787");
    let metadata = pcm_metadata("stream/server-open", 4);
    let control = StreamControl::Open {
        stream_id: metadata.id().clone(),
        metadata,
    };

    let missing_remote = transport
        .stream_control_frame(&mut cx, lisp_codec(), &control)
        .unwrap_err();
    assert!(matches!(
        missing_remote,
        sim_kernel::Error::CapabilityDenied { capability }
            if capability == stream_remote_network_capability()
    ));
    cx.grant(stream_remote_network_capability());

    let missing_operation = transport
        .stream_control_frame(&mut cx, lisp_codec(), &control)
        .unwrap_err();
    assert!(matches!(
        missing_operation,
        sim_kernel::Error::CapabilityDenied { capability }
            if capability == stream_open_capability()
    ));
    cx.grant(stream_open_capability());
    cx.grant(stream_stats_capability());

    let frame = transport
        .stream_control_frame(&mut cx, lisp_codec(), &control)
        .unwrap();
    let decoded = stream_control_from_frame(&mut cx, &frame).unwrap();

    assert_eq!(decoded, control);
    assert!(
        RemoteTransport::remote_server("https://sim.example")
            .stream_control_frame(
                &mut cx,
                lisp_codec(),
                &StreamControl::Stats {
                    stream_id: sym("stream/server-open")
                },
            )
            .is_ok()
    );
    assert!(
        RemoteTransport::wasm()
            .stream_control_frame(&mut cx, lisp_codec(), &control)
            .is_err()
    );
}

fn pcm_metadata(id: &str, capacity: usize) -> StreamMetadata {
    StreamMetadata::new(
        sym(id),
        StreamMedia::Pcm,
        StreamDirection::Source,
        ClockDomain::BrowserFrame.symbol(),
        BufferPolicy::bounded(capacity).unwrap(),
    )
}

fn pcm_metadata_with_overflow(id: &str, capacity: usize) -> StreamMetadata {
    StreamMetadata::new(
        sym(id),
        StreamMedia::Pcm,
        StreamDirection::Source,
        ClockDomain::BrowserFrame.symbol(),
        BufferPolicy::bounded_with_overflow(capacity, BufferOverflowPolicy::Error).unwrap(),
    )
}

fn pcm_item(value: f32) -> StreamItem {
    StreamItem::new(StreamPacket::Pcm(
        PcmPacket::f32(1, 1, vec![value]).unwrap(),
    ))
}

fn pcm_envelope(stream_id: &Symbol, sequence: u64, value: f32) -> StreamEnvelope {
    StreamEnvelope::new(
        stream_id.clone(),
        Symbol::qualified("stream/web-packet", format!("{stream_id}#{sequence}")),
        StreamMedia::Pcm,
        StreamDirection::Source,
        sequence,
        Vec::new(),
        ClockDomain::BrowserFrame,
        TransportProfile::lan_buffered_audio_preview(),
        Vec::new(),
        StreamPacket::Pcm(PcmPacket::f32(1, 1, vec![value]).unwrap()),
    )
    .unwrap()
}
