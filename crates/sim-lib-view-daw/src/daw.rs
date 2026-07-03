//! The DAW session lens: timeline, mixer, and plugin rack.
//!
//! The lens reads the existing `DawSession` model -- no second model -- and
//! renders a `scene/timeline` arrangement, mixer strips with `scene/meter`
//! meters and gain `scene/slider`s, and a plugin rack per track. Everything is
//! driven by Intents (scrub/set-param/invoke) committed via `realize`.

use sim_kernel::Expr;
use sim_lib_daw_session::{DawSession, DawTrack, instrument_session_render_smoke_command};
use sim_lib_scene::{data_map, node, sym};
use sim_value::build::uint;

/// The DAW session lens id.
pub const DAW_LENS: &str = "view:daw-timeline";

/// Render a DAW session into its lens Scene.
pub fn daw_view(session: &DawSession) -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("daw")),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![
                    transport_bar(session),
                    timeline(session),
                    mixer(session),
                    integration_panel(session),
                ]),
            ),
        ],
    )
}

fn transport_bar(session: &DawSession) -> Expr {
    let transport = session.transport();
    node(
        "box",
        vec![
            ("role", sym("transport")),
            (
                "children",
                Expr::List(vec![
                    node(
                        "text",
                        vec![(
                            "text",
                            Expr::String(format!("{:.1} bpm", transport.tempo_bpm())),
                        )],
                    ),
                    node(
                        "badge",
                        vec![
                            (
                                "status",
                                sym(if transport.playing() {
                                    "running"
                                } else {
                                    "idle"
                                }),
                            ),
                            (
                                "label",
                                Expr::String(
                                    if transport.playing() {
                                        "playing"
                                    } else {
                                        "stopped"
                                    }
                                    .to_owned(),
                                ),
                            ),
                        ],
                    ),
                    node(
                        "meter",
                        vec![
                            ("label", sym("position")),
                            ("value", uint(transport.sample_pos())),
                        ],
                    ),
                ]),
            ),
        ],
    )
}

fn timeline(session: &DawSession) -> Expr {
    let lanes = session
        .tracks()
        .iter()
        .map(|track| {
            let clips = track
                .clips()
                .iter()
                .map(|clip| {
                    data_map(vec![
                        ("id", Expr::Symbol(clip.id().clone())),
                        ("at", uint(clip.start_frame())),
                        ("len", uint(clip.frames())),
                    ])
                })
                .collect();
            data_map(vec![
                ("track", Expr::Symbol(track.id().clone())),
                ("name", Expr::String(track.name().to_owned())),
                ("clips", Expr::List(clips)),
            ])
        })
        .collect();
    node("timeline", vec![("lanes", Expr::List(lanes))])
}

fn mixer(session: &DawSession) -> Expr {
    let strips = session.tracks().iter().map(strip).collect();
    node(
        "stack",
        vec![
            ("role", sym("mixer")),
            ("dir", sym("row")),
            ("children", Expr::List(strips)),
        ],
    )
}

fn strip(track: &DawTrack) -> Expr {
    node(
        "box",
        vec![
            ("role", sym("strip")),
            (
                "children",
                Expr::List(vec![
                    node(
                        "text",
                        vec![("text", Expr::String(track.name().to_owned()))],
                    ),
                    node(
                        "slider",
                        vec![
                            ("param", sym("gain")),
                            ("min", number(0.0)),
                            ("max", number(1.0)),
                            ("value", number(0.8)),
                        ],
                    ),
                    node(
                        "meter",
                        vec![("label", sym("level")), ("value", number(0.0))],
                    ),
                    mute_solo(track),
                    plugin_rack(track),
                ]),
            ),
        ],
    )
}

fn mute_solo(track: &DawTrack) -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("mute-solo")),
            ("dir", sym("row")),
            (
                "children",
                Expr::List(vec![
                    node(
                        "badge",
                        vec![
                            (
                                "status",
                                sym(if track.is_muted() { "warn" } else { "idle" }),
                            ),
                            ("label", Expr::String("mute".to_owned())),
                        ],
                    ),
                    node(
                        "badge",
                        vec![
                            ("status", sym(if track.is_solo() { "ok" } else { "idle" })),
                            ("label", Expr::String("solo".to_owned())),
                        ],
                    ),
                ]),
            ),
        ],
    )
}

fn plugin_rack(track: &DawTrack) -> Expr {
    let slots = track
        .plugin_chain()
        .slots()
        .iter()
        .map(|slot| {
            node(
                "box",
                vec![
                    ("role", sym("plugin")),
                    (
                        "children",
                        Expr::List(vec![
                            node("text", vec![("text", Expr::String(slot.id().to_string()))]),
                            node(
                                "badge",
                                vec![
                                    (
                                        "status",
                                        sym(if slot.is_bypassed() { "warn" } else { "ok" }),
                                    ),
                                    (
                                        "label",
                                        Expr::String(
                                            if slot.is_bypassed() {
                                                "bypassed"
                                            } else {
                                                "active"
                                            }
                                            .to_owned(),
                                        ),
                                    ),
                                ],
                            ),
                        ]),
                    ),
                ],
            )
        })
        .collect();
    node(
        "stack",
        vec![
            ("role", sym("rack")),
            ("dir", sym("column")),
            ("children", Expr::List(slots)),
        ],
    )
}

fn integration_panel(session: &DawSession) -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("instrument-integration")),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![
                    node(
                        "table",
                        vec![
                            ("role", sym("instrument-instances")),
                            (
                                "rows",
                                Expr::List(
                                    session
                                        .instrument_instances()
                                        .iter()
                                        .map(|instrument| {
                                            data_map(vec![
                                                ("id", Expr::Symbol(instrument.id().clone())),
                                                (
                                                    "instrument-kind",
                                                    sym(instrument.kind().as_str()),
                                                ),
                                                (
                                                    "node",
                                                    Expr::String(
                                                        instrument.graph_node_id().to_owned(),
                                                    ),
                                                ),
                                                (
                                                    "fixture",
                                                    Expr::String(
                                                        instrument.patch_fixture().to_owned(),
                                                    ),
                                                ),
                                            ])
                                        })
                                        .collect(),
                                ),
                            ),
                        ],
                    ),
                    node(
                        "table",
                        vec![
                            ("role", sym("instrument-route-matrix")),
                            (
                                "rows",
                                Expr::List(
                                    session
                                        .routes()
                                        .iter()
                                        .map(|route| {
                                            data_map(vec![
                                                ("route-kind", sym(route.kind().as_str())),
                                                ("source", Expr::Symbol(route.source().clone())),
                                                (
                                                    "node",
                                                    Expr::String(route.target_node_id().to_owned()),
                                                ),
                                                ("target", Expr::Symbol(route.target().clone())),
                                            ])
                                        })
                                        .collect(),
                                ),
                            ),
                        ],
                    ),
                    node(
                        "text",
                        vec![
                            ("role", sym("render-smoke-command")),
                            (
                                "text",
                                Expr::String(instrument_session_render_smoke_command().to_owned()),
                            ),
                        ],
                    ),
                ]),
            ),
        ],
    )
}

use sim_value::build::float as number;
