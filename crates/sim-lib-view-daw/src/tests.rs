//! Tests for the DAW and synth lenses.

use sim_kernel::{Expr, Symbol};
use sim_lib_daw_session::{DawClip, DawSession, DawTrack, instrument_session_fixture};
use sim_lib_stream_core::{
    BufferPolicy, PcmPacket, StreamDirection, StreamInspectorSnapshot, StreamInspectorStatus,
    StreamItem, StreamMedia, StreamMetadata, StreamPacket, StreamStats, TransportProfile,
    stream_inspector_route_local_symbol,
};

use crate::component::{
    COMPONENT_EDITOR_INVALID_VALUE_FIXTURE, COMPONENT_EDITOR_MANY_PARAM_FIXTURE,
    COMPONENT_EDITOR_NO_PARAM_FIXTURE, COMPONENT_EDITOR_TRACE_ONLY_FIXTURE,
    COMPONENT_EDITOR_VIEW_ID, component_editor_fixture_names, component_editor_snapshot,
};
use crate::daw::daw_view;
use crate::modular::{
    COMPONENT_BUILDER_ACTIONS, COMPONENT_BUILDER_CORD_EDIT_FIXTURE,
    COMPONENT_BUILDER_GRAPH_EDIT_FIXTURE, COMPONENT_BUILDER_INVALID_PATCH_FIXTURE,
    COMPONENT_BUILDER_PATCH_FORMAT, COMPONENT_BUILDER_SECTION_EDIT_FIXTURE,
    COMPONENT_BUILDER_VALIDATION_CODES, COMPONENT_BUILDER_VIEW_ID, component_builder_fixture_names,
    component_builder_snapshot,
};
use crate::param::{apply_scrub, apply_set_param};
use crate::specialized::{
    ALGORITHM_ROUTING_FIXTURE, ENVELOPE_CURVE_FIXTURE, FILTER_RESPONSE_FIXTURE,
    FIXED_FILTER_BANK_FIXTURE, POLYPHONY_ACTIVITY_FIXTURE, RESONATOR_RESPONSE_FIXTURE,
    SCOPE_SPECTRUM_FIXTURE, SEQUENCER_STEP_GRID_FIXTURE, SPECIALIZED_COMPONENT_VIEW_IDS,
    SYSEX_COMPARISON_FIXTURE, specialized_declaring_components, specialized_fixture_names,
    specialized_snapshot, specialized_view_ids,
};
use crate::stream::{
    STREAM_DETAIL_VIEW_ID, STREAM_DIAGNOSTIC_TIMELINE_VIEW_ID, STREAM_LIST_VIEW_ID,
    STREAM_PACKET_PREVIEW_VIEW_ID, stream_detail_view, stream_diagnostic_timeline_view,
    stream_list_view, stream_packet_preview_view,
};
use crate::synth::{spectrum_view, synth_panel, waveform_view};

use sim_value::build::sym;

use sim_value::build::float as number;

fn session() -> DawSession {
    let mut session = DawSession::new("sess", "My Session", 48_000).unwrap();
    let track = DawTrack::audio("lead", "Lead", 2)
        .unwrap()
        .with_clip(DawClip::silence("c1", 0, 48_000).unwrap());
    session.add_track(track).unwrap();
    session
}

fn synth_params() -> Expr {
    Expr::Map(vec![
        (sym("cutoff"), number(0.5)),
        (sym("resonance"), number(0.2)),
    ])
}

#[test]
fn a_daw_session_opens_in_a_specialized_lens() {
    let session = session();
    let scene = daw_view(&session);
    sim_lib_scene::validate_scene(&scene).expect("the DAW scene is valid");
    assert!(
        sim_test_support::contains_kind(&scene, "timeline"),
        "the lens has a timeline"
    );
    assert!(
        sim_test_support::contains_kind(&scene, "meter"),
        "the lens has live meters"
    );
}

#[test]
fn daw_instrument_session_exposes_routes_and_smoke_command() {
    let session = instrument_session_fixture();
    let scene = daw_view(&session);
    sim_lib_scene::validate_scene(&scene).expect("the instrument session scene is valid");

    assert!(contains_role(&scene, "instrument-integration"));
    assert!(contains_role(&scene, "instrument-instances"));
    assert!(contains_role(&scene, "instrument-route-matrix"));
    assert!(contains_role(&scene, "render-smoke-command"));
    assert!(contains_symbol(&scene, "daw-route-source", "midi-lead"));
    assert!(contains_symbol(&scene, "daw-route-target", "preview"));
    assert!(contains_text(
        &scene,
        "cargo test -p sim-lib-daw-session instrument_session_load_render_reopen_smoke"
    ));
}

#[test]
fn a_synth_patch_opens_with_knobs_and_signal_displays() {
    let panel = synth_panel(&synth_params());
    sim_lib_scene::validate_scene(&panel).expect("the synth panel is valid");
    assert!(
        sim_test_support::contains_kind(&panel, "knob"),
        "the synth has knobs"
    );
    assert!(
        sim_test_support::contains_kind(&panel, "matrix"),
        "the synth has a modulation matrix"
    );

    let wave = waveform_view(&[0.0, 0.5, 1.0, 0.5, 0.0]);
    sim_lib_scene::validate_scene(&wave).expect("the waveform is valid");
    let spectrum = spectrum_view(&[1.0, 0.5, 0.25]);
    sim_lib_scene::validate_scene(&spectrum).expect("the spectrum is valid");
}

#[test]
fn a_parameter_change_commits_a_new_value() {
    let params = synth_params();
    let intent = sim_lib_intent::intent(
        "set-param",
        sim_lib_intent::Origin::human(1),
        vec![
            ("target", sym("synth")),
            ("param", sym("cutoff")),
            ("value", number(0.9)),
        ],
    );
    let committed = apply_set_param(&params, &intent).unwrap();
    // The operation value carries the changed parameter; the original is intact.
    assert_eq!(field(&committed, "cutoff"), Some(number(0.9)));
    assert_eq!(field(&committed, "resonance"), Some(number(0.2)));
    assert_eq!(field(&params, "cutoff"), Some(number(0.5)));
}

#[test]
fn scrubbing_moves_the_transport_playhead() {
    let transport = Expr::Map(vec![(sym("position"), number(0.0))]);
    let intent = sim_lib_intent::intent(
        "scrub",
        sim_lib_intent::Origin::human(1),
        vec![("target", sym("transport")), ("at", number(1024.0))],
    );
    let moved = apply_scrub(&transport, &intent).unwrap();
    assert_eq!(field(&moved, "position"), Some(number(1024.0)));
}

#[test]
fn stream_inspector_views_have_stable_scene_ids() {
    let snapshot = stream_snapshot();
    let packet = StreamItem::new(StreamPacket::Pcm(PcmPacket::f32(1, 1, vec![0.25]).unwrap()));

    let list = stream_list_view(std::slice::from_ref(&snapshot));
    let detail = stream_detail_view(&snapshot);
    let preview = stream_packet_preview_view(&[packet]);
    let timeline = stream_diagnostic_timeline_view(&snapshot);

    for scene in [&list, &detail, &preview, &timeline] {
        sim_lib_scene::validate_scene(scene).expect("stream inspector scene is valid");
    }
    assert!(sim_test_support::contains_kind(&list, "stack"));
    assert!(sim_test_support::contains_kind(&detail, "box"));
    assert!(sim_test_support::contains_kind(&preview, "table"));
    assert!(sim_test_support::contains_kind(&timeline, "timeline"));
    assert_eq!(field(&list, "lens"), Some(sym(STREAM_LIST_VIEW_ID)));
    assert_eq!(field(&detail, "lens"), Some(sym(STREAM_DETAIL_VIEW_ID)));
    assert_eq!(
        field(&preview, "lens"),
        Some(sym(STREAM_PACKET_PREVIEW_VIEW_ID))
    );
    assert_eq!(
        field(&timeline, "lens"),
        Some(sym(STREAM_DIAGNOSTIC_TIMELINE_VIEW_ID))
    );
    assert_eq!(field(&list, "role"), Some(sym("stream-list")));
    assert_eq!(field(&detail, "role"), Some(sym("stream-detail")));
    assert_eq!(field(&preview, "role"), Some(sym("stream-packet-preview")));
    assert_eq!(
        field(&timeline, "role"),
        Some(sym("stream-diagnostic-timeline"))
    );
}

#[test]
fn component_editor_snapshots_cover_generic_fixture_shapes() {
    assert_eq!(
        component_editor_fixture_names(),
        [
            COMPONENT_EDITOR_MANY_PARAM_FIXTURE,
            COMPONENT_EDITOR_NO_PARAM_FIXTURE,
            COMPONENT_EDITOR_INVALID_VALUE_FIXTURE,
            COMPONENT_EDITOR_TRACE_ONLY_FIXTURE,
        ]
    );

    for name in component_editor_fixture_names() {
        let scene = component_editor_snapshot(name).expect("component editor fixture");
        sim_lib_scene::validate_scene(&scene).expect("component editor scene is valid");
        assert_eq!(scene, component_editor_snapshot(name).unwrap());
        assert_eq!(field(&scene, "lens"), Some(sym(COMPONENT_EDITOR_VIEW_ID)));
        assert_eq!(field(&scene, "role"), Some(sym("component-editor")));
    }

    let many = component_editor_snapshot(COMPONENT_EDITOR_MANY_PARAM_FIXTURE).unwrap();
    for editor in [
        "integer-range",
        "enum",
        "toggle",
        "fixed-point",
        "normalized",
    ] {
        assert!(contains_editor(&many, editor), "missing editor {editor}");
    }
    assert!(contains_role(&many, "specialized-route"));

    let no_param = component_editor_snapshot(COMPONENT_EDITOR_NO_PARAM_FIXTURE).unwrap();
    assert!(contains_role(&no_param, "parameter-empty"));

    let invalid = component_editor_snapshot(COMPONENT_EDITOR_INVALID_VALUE_FIXTURE).unwrap();
    assert!(contains_role(&invalid, "validation-errors"));
    assert!(contains_role(&invalid, "param-error"));

    let trace_only = component_editor_snapshot(COMPONENT_EDITOR_TRACE_ONLY_FIXTURE).unwrap();
    assert_eq!(
        field(&trace_only, "trace-available"),
        Some(Expr::Bool(true))
    );
    assert!(contains_editor(&trace_only, "trace-readonly"));
}

#[test]
fn component_builder_snapshots_cover_graph_cord_section_and_invalid_patch() {
    assert_eq!(
        component_builder_fixture_names(),
        [
            COMPONENT_BUILDER_GRAPH_EDIT_FIXTURE,
            COMPONENT_BUILDER_CORD_EDIT_FIXTURE,
            COMPONENT_BUILDER_SECTION_EDIT_FIXTURE,
            COMPONENT_BUILDER_INVALID_PATCH_FIXTURE,
        ]
    );

    for name in component_builder_fixture_names() {
        let scene = component_builder_snapshot(name).expect("component builder fixture");
        sim_lib_scene::validate_scene(&scene).expect("component builder scene is valid");
        assert_eq!(scene, component_builder_snapshot(name).unwrap());
        assert_eq!(field(&scene, "lens"), Some(sym(COMPONENT_BUILDER_VIEW_ID)));
        assert_eq!(field(&scene, "role"), Some(sym("component-builder")));
        assert_eq!(
            field(&scene, "patch-format"),
            Some(Expr::String(COMPONENT_BUILDER_PATCH_FORMAT.to_owned()))
        );
        for action in COMPONENT_BUILDER_ACTIONS {
            assert!(contains_action(&scene, action), "missing action {action}");
        }
    }

    let graph = component_builder_snapshot(COMPONENT_BUILDER_GRAPH_EDIT_FIXTURE).unwrap();
    assert!(contains_role(&graph, "component-palette"));
    assert!(contains_role(&graph, "component-graph"));
    assert!(contains_role(&graph, "component-cord-editor"));
    assert!(contains_role(&graph, "poly-section-view"));
    assert!(contains_role(&graph, "builder-persistence"));
    assert!(contains_symbol(&graph, "audio-synth/component", "r700-vco"));
    assert!(contains_symbol(&graph, "audio-synth/component", "dx7"));
    assert!(!contains_symbol(&graph, "audio-synth/component", "adapter"));

    let cord = component_builder_snapshot(COMPONENT_BUILDER_CORD_EDIT_FIXTURE).unwrap();
    assert!(contains_action(&cord, "route-matrix"));
    assert!(contains_action(&cord, "connect"));
    assert!(contains_action(&cord, "disconnect"));

    let section = component_builder_snapshot(COMPONENT_BUILDER_SECTION_EDIT_FIXTURE).unwrap();
    assert!(contains_action(&section, "enable-section"));
    assert!(contains_action(&section, "disable-section"));

    let invalid = component_builder_snapshot(COMPONENT_BUILDER_INVALID_PATCH_FIXTURE).unwrap();
    assert!(contains_role(&invalid, "builder-validation"));
    for code in COMPONENT_BUILDER_VALIDATION_CODES {
        assert!(
            contains_validation(&invalid, code),
            "missing validation {code}"
        );
    }
}

#[test]
fn specialized_snapshots_cover_declared_component_views() {
    assert_eq!(
        specialized_fixture_names(),
        [
            ENVELOPE_CURVE_FIXTURE,
            ALGORITHM_ROUTING_FIXTURE,
            FILTER_RESPONSE_FIXTURE,
            RESONATOR_RESPONSE_FIXTURE,
            FIXED_FILTER_BANK_FIXTURE,
            SEQUENCER_STEP_GRID_FIXTURE,
            POLYPHONY_ACTIVITY_FIXTURE,
            SCOPE_SPECTRUM_FIXTURE,
            SYSEX_COMPARISON_FIXTURE,
        ]
    );
    assert_eq!(
        specialized_view_ids(),
        SPECIALIZED_COMPONENT_VIEW_IDS.as_slice()
    );

    for view_id in specialized_view_ids() {
        assert!(
            specialized_declaring_components()
                .iter()
                .any(|declaration| declaration.view_id == *view_id),
            "missing declaring component for {view_id}"
        );
    }

    for name in specialized_fixture_names() {
        let scene = specialized_snapshot(name).expect("specialized fixture");
        sim_lib_scene::validate_scene(&scene).expect("specialized scene is valid");
        assert_eq!(scene, specialized_snapshot(name).unwrap());
        assert!(fixture_lens_is_declared(&scene), "fixture lens is declared");
        assert!(
            matches!(field(&scene, "declaring-components"), Some(Expr::List(items)) if !items.is_empty()),
            "fixture records declaring components"
        );
    }

    let envelope = specialized_snapshot(ENVELOPE_CURVE_FIXTURE).unwrap();
    assert!(contains_role(&envelope, "envelope-curve-editor"));
    assert!(contains_role(&envelope, "envelope-point-editor"));

    let algorithm = specialized_snapshot(ALGORITHM_ROUTING_FIXTURE).unwrap();
    assert!(contains_role(&algorithm, "algorithm-routing-matrix"));
    assert!(contains_role(&algorithm, "wiring-diagram"));

    let filter = specialized_snapshot(FILTER_RESPONSE_FIXTURE).unwrap();
    assert!(contains_role(&filter, "filter-response-plot"));
    assert!(contains_role(&filter, "response-sweep"));

    let resonator = specialized_snapshot(RESONATOR_RESPONSE_FIXTURE).unwrap();
    assert!(contains_role(&resonator, "resonator-response-plot"));

    let bank = specialized_snapshot(FIXED_FILTER_BANK_FIXTURE).unwrap();
    assert!(contains_role(&bank, "fixed-filter-bank-view"));
    assert!(contains_role(&bank, "fixed-filter-bank-bands"));

    let sequencer = specialized_snapshot(SEQUENCER_STEP_GRID_FIXTURE).unwrap();
    assert!(contains_role(&sequencer, "sequencer-step-grid"));

    let polyphony = specialized_snapshot(POLYPHONY_ACTIVITY_FIXTURE).unwrap();
    assert!(contains_role(&polyphony, "polyphony-activity-map"));

    let monitor = specialized_snapshot(SCOPE_SPECTRUM_FIXTURE).unwrap();
    assert!(contains_role(&monitor, "scope-spectrum-monitor"));
    assert!(contains_role(&monitor, "scope-monitor"));
    assert!(contains_role(&monitor, "spectrum-monitor"));

    let sysex = specialized_snapshot(SYSEX_COMPARISON_FIXTURE).unwrap();
    assert!(contains_role(&sysex, "sysex-comparison-view"));
    assert!(contains_role(&sysex, "round-trip-probe"));
}

fn field(map: &Expr, name: &str) -> Option<Expr> {
    let Expr::Map(entries) = map else { return None };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(s) if &*s.name == name).then(|| value.clone())
    })
}

fn fixture_lens_is_declared(scene: &Expr) -> bool {
    matches!(
        field(scene, "lens"),
        Some(Expr::Symbol(symbol)) if SPECIALIZED_COMPONENT_VIEW_IDS.contains(&symbol.name.as_ref())
    )
}

fn contains_role(expr: &Expr, role: &str) -> bool {
    field(expr, "role") == Some(sym(role))
        || expr_children(expr)
            .iter()
            .any(|child| contains_role(child, role))
}

fn contains_editor(expr: &Expr, editor: &str) -> bool {
    field(expr, "editor")
        == Some(Expr::Symbol(Symbol::qualified(
            "component-editor/editor",
            editor,
        )))
        || expr_children(expr)
            .iter()
            .any(|child| contains_editor(child, editor))
}

fn contains_action(expr: &Expr, action: &str) -> bool {
    field(expr, "action") == Some(sym(action))
        || expr_children(expr)
            .iter()
            .any(|child| contains_action(child, action))
}

fn contains_validation(expr: &Expr, code: &str) -> bool {
    match field(expr, "validation") {
        Some(Expr::Symbol(symbol))
            if symbol.namespace.as_deref() == Some("component-builder/validation")
                && symbol.name.as_ref() == code =>
        {
            true
        }
        _ => expr_children(expr)
            .iter()
            .any(|child| contains_validation(child, code)),
    }
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

fn contains_text(expr: &Expr, expected: &str) -> bool {
    match expr {
        Expr::String(text) if text == expected => true,
        _ => expr_children(expr)
            .iter()
            .any(|child| contains_text(child, expected)),
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

fn stream_snapshot() -> StreamInspectorSnapshot {
    let metadata = StreamMetadata::new(
        Symbol::new("stream/view"),
        StreamMedia::Pcm,
        StreamDirection::Source,
        Symbol::qualified("clock", "sample"),
        BufferPolicy::bounded(4).unwrap(),
    );
    StreamInspectorSnapshot::new(
        &metadata,
        stream_inspector_route_local_symbol(),
        TransportProfile::memory_local().name().clone(),
        StreamInspectorStatus::BufferOverflow,
        2,
        &StreamStats {
            dropped_newest: 1,
            ..StreamStats::default()
        },
        Some(7),
        vec![Symbol::qualified("stream/diagnostic", "drop")],
    )
}
