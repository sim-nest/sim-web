//! Arranger object-roll Scene descriptors.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};
use sim_value::build::{int, list, text, uint};

/// Stable lens id for the arranger object-roll editor.
pub const ARRANGER_OBJECT_ROLL_VIEW_ID: &str = "view:arranger-object-roll";

/// Demo fixture name for the arranger object-roll editor.
pub const ARRANGER_OBJECT_ROLL_DEMO_FIXTURE: &str = "arranger-object-roll";

/// Editing actions exposed by the object-roll editor.
pub const ARRANGER_OBJECT_ROLL_ACTIONS: &[&str] = &[
    "set-at",
    "set-duration",
    "set-stretch",
    "set-transform",
    "set-remap-pitch",
    "set-filter",
    "set-target",
    "set-seed",
    "set-trace-policy",
    "open-nested",
    "freeze-to-piano-roll",
    "freeze-to-midi",
];

/// One visible object-roll lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrangerLane {
    /// Stable lane id.
    pub id: Symbol,
    /// Display label.
    pub label: String,
    /// Placement cells in this lane.
    pub placements: Vec<ArrangerObjectRollPlacement>,
}

/// One arranger placement cell shown by the object-roll editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrangerObjectRollPlacement {
    /// Stable placement id.
    pub id: Symbol,
    /// Display label.
    pub label: String,
    /// Lane that owns this placement.
    pub lane: Symbol,
    /// Playable object or reference.
    pub playable: Symbol,
    /// Start tick.
    pub at: u64,
    /// Duration in ticks.
    pub duration: u64,
    /// Stretch policy label.
    pub stretch: String,
    /// Transposition in semitones.
    pub transpose: i32,
    /// Inversion handle label.
    pub invert: String,
    /// Retrograde transform toggle.
    pub retrograde: bool,
    /// Pitch remap handle label.
    pub remap_pitch: String,
    /// Filter object.
    pub filter: Symbol,
    /// Target instrument, lane, or playable sink.
    pub target: Symbol,
    /// Deterministic seed for generative placements.
    pub seed: u64,
    /// Trace policy label.
    pub trace_policy: String,
    /// Whether this placement opens another arranger.
    pub nested: bool,
}

/// Diagnostic class rendered by the object-roll editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrangerDiagnosticKind {
    /// A source event could not be represented in the current lane.
    DroppedEvent,
    /// A target does not provide a required capability.
    MissingCapability,
    /// A pitch remap cannot be applied.
    ImpossibleRemap,
    /// A placement range is clipped by the edit range.
    ClippedRange,
}

impl ArrangerDiagnosticKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::DroppedEvent => "dropped-event",
            Self::MissingCapability => "missing-capability",
            Self::ImpossibleRemap => "impossible-remap",
            Self::ClippedRange => "clipped-range",
        }
    }
}

/// One object-roll diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrangerDiagnostic {
    /// Related placement id.
    pub placement: Symbol,
    /// Diagnostic class.
    pub diagnostic_kind: ArrangerDiagnosticKind,
    /// Short display message.
    pub message: String,
}

/// Complete arranger object-roll view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrangerObjectRollView {
    /// Intent target edited by arranger actions.
    pub target: Symbol,
    /// Arranger object being edited.
    pub arranger: Symbol,
    /// Visible lanes.
    pub lanes: Vec<ArrangerLane>,
    /// Diagnostics to display next to placements.
    pub diagnostics: Vec<ArrangerDiagnostic>,
}

/// Render an arranger descriptor as a `scene/object-roll` node.
pub fn arranger_object_roll_view(view: &ArrangerObjectRollView) -> Expr {
    node(
        "object-roll",
        vec![
            ("lens", sym(ARRANGER_OBJECT_ROLL_VIEW_ID)),
            ("role", sym("arranger-object-roll")),
            ("target", Expr::Symbol(view.target.clone())),
            ("arranger", Expr::Symbol(view.arranger.clone())),
            (
                "actions",
                list(
                    ARRANGER_OBJECT_ROLL_ACTIONS
                        .iter()
                        .map(|action| text(*action))
                        .collect(),
                ),
            ),
            (
                "lanes",
                list(view.lanes.iter().map(arranger_lane_expr).collect()),
            ),
            (
                "diagnostics",
                list(
                    view.diagnostics
                        .iter()
                        .map(arranger_diagnostic_expr)
                        .collect(),
                ),
            ),
        ],
    )
}

/// Deterministic arranger object-roll fixture covering transform handles.
pub fn arranger_object_roll_demo_view() -> ArrangerObjectRollView {
    let melody_lane = Symbol::qualified("music/arranger-lane", "melody");
    let nested_lane = Symbol::qualified("music/arranger-lane", "nested");
    let automation_lane = Symbol::qualified("music/arranger-lane", "automation");
    let motif = ArrangerObjectRollPlacement {
        id: Symbol::qualified("music/arranger-placement", "motif"),
        label: "Motif".to_owned(),
        lane: melody_lane.clone(),
        playable: Symbol::qualified("music/playable", "motif-roll"),
        at: 0,
        duration: 384,
        stretch: "fit-to-duration".to_owned(),
        transpose: 12,
        invert: "pitch:C4".to_owned(),
        retrograde: true,
        remap_pitch: "scale:minor-pentatonic".to_owned(),
        filter: Symbol::qualified("music/filter", "lead-only"),
        target: Symbol::qualified("audio-synth/instrument", "dx7"),
        seed: 9001,
        trace_policy: "full".to_owned(),
        nested: false,
    };
    let nested = ArrangerObjectRollPlacement {
        id: Symbol::qualified("music/arranger-placement", "nested-arranger"),
        label: "Nested arranger".to_owned(),
        lane: nested_lane.clone(),
        playable: Symbol::qualified("music/arranger", "bridge"),
        at: 384,
        duration: 384,
        stretch: "tempo-ratio:3/2".to_owned(),
        transpose: 0,
        invert: "none".to_owned(),
        retrograde: false,
        remap_pitch: "vector:modal-axis".to_owned(),
        filter: Symbol::qualified("music/filter", "none"),
        target: Symbol::qualified("music/player-chain", "onscreen-keyboard"),
        seed: 17,
        trace_policy: "diagnostics".to_owned(),
        nested: true,
    };
    let automation = ArrangerObjectRollPlacement {
        id: Symbol::qualified("music/arranger-placement", "cutoff-sweep"),
        label: "Cutoff sweep".to_owned(),
        lane: automation_lane.clone(),
        playable: Symbol::qualified("music/playable", "cutoff-curve"),
        at: 768,
        duration: 192,
        stretch: "none".to_owned(),
        transpose: 0,
        invert: "none".to_owned(),
        retrograde: false,
        remap_pitch: "matrix:ps3300-map".to_owned(),
        filter: Symbol::qualified("music/filter", "controls"),
        target: Symbol::qualified("audio-synth/parameter", "cutoff"),
        seed: 5,
        trace_policy: "off".to_owned(),
        nested: false,
    };
    ArrangerObjectRollView {
        target: Symbol::qualified("music/arranger", "song-a"),
        arranger: Symbol::qualified("music/arranger", "song-a"),
        lanes: vec![
            ArrangerLane {
                id: melody_lane,
                label: "Melody".to_owned(),
                placements: vec![motif.clone()],
            },
            ArrangerLane {
                id: nested_lane,
                label: "Nested".to_owned(),
                placements: vec![nested.clone()],
            },
            ArrangerLane {
                id: automation_lane,
                label: "Automation".to_owned(),
                placements: vec![automation.clone()],
            },
        ],
        diagnostics: vec![
            ArrangerDiagnostic {
                placement: motif.id.clone(),
                diagnostic_kind: ArrangerDiagnosticKind::DroppedEvent,
                message: "dropped control event".to_owned(),
            },
            ArrangerDiagnostic {
                placement: motif.id,
                diagnostic_kind: ArrangerDiagnosticKind::MissingCapability,
                message: "target lacks pitch input".to_owned(),
            },
            ArrangerDiagnostic {
                placement: nested.id,
                diagnostic_kind: ArrangerDiagnosticKind::ImpossibleRemap,
                message: "vector remap misses row".to_owned(),
            },
            ArrangerDiagnostic {
                placement: automation.id,
                diagnostic_kind: ArrangerDiagnosticKind::ClippedRange,
                message: "placement clipped at loop end".to_owned(),
            },
        ],
    }
}

/// Deterministic arranger object-roll demo scene.
pub fn arranger_object_roll_demo_scene() -> Expr {
    arranger_object_roll_view(&arranger_object_roll_demo_view())
}

fn arranger_lane_expr(lane: &ArrangerLane) -> Expr {
    data_map(vec![
        ("id", Expr::Symbol(lane.id.clone())),
        ("label", text(lane.label.clone())),
        (
            "placements",
            list(
                lane.placements
                    .iter()
                    .map(arranger_placement_expr)
                    .collect(),
            ),
        ),
    ])
}

fn arranger_placement_expr(placement: &ArrangerObjectRollPlacement) -> Expr {
    data_map(vec![
        ("id", Expr::Symbol(placement.id.clone())),
        ("label", text(placement.label.clone())),
        ("lane", Expr::Symbol(placement.lane.clone())),
        ("playable", Expr::Symbol(placement.playable.clone())),
        ("at", uint(placement.at)),
        ("duration", uint(placement.duration)),
        ("stretch", text(placement.stretch.clone())),
        ("transpose", int(i64::from(placement.transpose))),
        ("invert", text(placement.invert.clone())),
        ("retrograde", Expr::Bool(placement.retrograde)),
        ("remap-pitch", text(placement.remap_pitch.clone())),
        ("filter", Expr::Symbol(placement.filter.clone())),
        ("target", Expr::Symbol(placement.target.clone())),
        ("seed", uint(placement.seed)),
        ("trace-policy", text(placement.trace_policy.clone())),
        ("nested", Expr::Bool(placement.nested)),
        (
            "freeze-targets",
            list(vec![text("piano-roll"), text("midi")]),
        ),
    ])
}

fn arranger_diagnostic_expr(diagnostic: &ArrangerDiagnostic) -> Expr {
    data_map(vec![
        ("placement", Expr::Symbol(diagnostic.placement.clone())),
        ("diagnostic-kind", text(diagnostic.diagnostic_kind.as_str())),
        ("message", text(diagnostic.message.clone())),
    ])
}
