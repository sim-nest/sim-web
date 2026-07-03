//! Player-rack Scene descriptors for live player chains.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};
use sim_value::build::{list, text, uint};

use crate::keyboard::performance_keyboard_demo_scene;
use crate::piano_roll::piano_roll_demo_scene;

/// Stable lens id for the player-rack view.
pub const PLAYER_RACK_VIEW_ID: &str = "view:player-rack";

/// Editing actions exposed by the player-rack scene.
pub const PLAYER_RACK_ACTIONS: &[&str] = &[
    "add",
    "remove",
    "reorder",
    "bypass",
    "direct-record",
    "freeze",
    "trace",
    "route",
    "placement-hint",
];

/// One player device in a rack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerRackDevice {
    /// Stable player id.
    pub id: Symbol,
    /// Display label.
    pub label: String,
    /// Player family.
    pub player_kind: Symbol,
    /// Zero-based order in the chain.
    pub order: u64,
    /// Bypass state.
    pub bypassed: bool,
    /// Whether direct recording is armed.
    pub direct_record: bool,
    /// Whether output is frozen for deterministic replay.
    pub frozen: bool,
    /// Whether trace inspection is enabled.
    pub trace: bool,
    /// Routing target after the player.
    pub route: Symbol,
    /// Placement hint for this player.
    pub placement_hint: String,
}

/// Complete player-rack descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerRackView {
    /// Intent target edited by rack actions.
    pub target: Symbol,
    /// Player chain represented by the rack.
    pub player_chain: Symbol,
    /// Instrument target at the end of the chain.
    pub instrument: Symbol,
    /// Performance source feeding the chain.
    pub source: Symbol,
    /// Stream bridge route for live capture.
    pub stream: Symbol,
    /// Rack-level placement hint.
    pub placement_hint: String,
    /// Player devices.
    pub players: Vec<PlayerRackDevice>,
}

/// Render a player rack as `scene/player-rack`.
pub fn player_rack_view(view: &PlayerRackView) -> Expr {
    node(
        "player-rack",
        vec![
            ("lens", sym(PLAYER_RACK_VIEW_ID)),
            ("role", sym("player-rack")),
            ("target", Expr::Symbol(view.target.clone())),
            ("player-chain", Expr::Symbol(view.player_chain.clone())),
            ("instrument", Expr::Symbol(view.instrument.clone())),
            ("source", Expr::Symbol(view.source.clone())),
            ("stream", Expr::Symbol(view.stream.clone())),
            ("placement-hint", text(view.placement_hint.clone())),
            (
                "actions",
                list(
                    PLAYER_RACK_ACTIONS
                        .iter()
                        .map(|action| text(*action))
                        .collect(),
                ),
            ),
            (
                "players",
                list(view.players.iter().map(player_rack_device_expr).collect()),
            ),
        ],
    )
}

/// Deterministic rack fixture over the keyboard performance source.
pub fn player_rack_demo_view() -> PlayerRackView {
    let chain = Symbol::qualified("music/player-chain", "onscreen-keyboard");
    let source = Symbol::qualified("music/performance-source", "keyboard");
    PlayerRackView {
        target: chain.clone(),
        player_chain: chain.clone(),
        instrument: Symbol::qualified("audio-synth/instrument", "dx7"),
        source,
        stream: Symbol::qualified("stream/browser", "performance-keyboard"),
        placement_hint: "browser-wasm".to_owned(),
        players: vec![
            PlayerRackDevice {
                id: Symbol::qualified("music/player", "scales-chords"),
                label: "Scales and chords".to_owned(),
                player_kind: Symbol::qualified("music/player-kind", "scales-chords"),
                order: 0,
                bypassed: false,
                direct_record: false,
                frozen: false,
                trace: true,
                route: chain.clone(),
                placement_hint: "browser-wasm".to_owned(),
            },
            PlayerRackDevice {
                id: Symbol::qualified("music/player", "dual-arpeggio"),
                label: "Dual arpeggio".to_owned(),
                player_kind: Symbol::qualified("music/player-kind", "dual-arpeggio"),
                order: 1,
                bypassed: false,
                direct_record: true,
                frozen: false,
                trace: true,
                route: chain.clone(),
                placement_hint: "worker".to_owned(),
            },
            PlayerRackDevice {
                id: Symbol::qualified("music/player", "note-echo"),
                label: "Note echo".to_owned(),
                player_kind: Symbol::qualified("music/player-kind", "note-echo"),
                order: 2,
                bypassed: true,
                direct_record: false,
                frozen: true,
                trace: false,
                route: Symbol::qualified("audio-synth/instrument", "dx7"),
                placement_hint: "audio-worklet".to_owned(),
            },
        ],
    }
}

/// Deterministic player-rack demo scene.
pub fn player_rack_demo_scene() -> Expr {
    player_rack_view(&player_rack_demo_view())
}

/// Shell demo with keyboard input feeding a rack and piano roll.
pub fn performance_workbench_demo_scene() -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("performance-workbench")),
            ("dir", sym("column")),
            (
                "children",
                list(vec![
                    performance_keyboard_demo_scene(),
                    player_rack_demo_scene(),
                    piano_roll_demo_scene(),
                ]),
            ),
        ],
    )
}

fn player_rack_device_expr(device: &PlayerRackDevice) -> Expr {
    data_map(vec![
        ("id", Expr::Symbol(device.id.clone())),
        ("label", text(device.label.clone())),
        ("player-kind", Expr::Symbol(device.player_kind.clone())),
        ("order", uint(device.order)),
        ("bypassed", Expr::Bool(device.bypassed)),
        ("direct-record", Expr::Bool(device.direct_record)),
        ("frozen", Expr::Bool(device.frozen)),
        ("trace", Expr::Bool(device.trace)),
        ("route", Expr::Symbol(device.route.clone())),
        ("placement-hint", text(device.placement_hint.clone())),
    ])
}
