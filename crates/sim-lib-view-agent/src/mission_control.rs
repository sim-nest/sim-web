//! Mission Control Scene for agent-operated work.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};
use sim_lib_stream_core::{DevCassette, StreamPacket};
use sim_value::build::uint;

/// Mission Control lens id.
pub const MISSION_CONTROL_LENS: &str = "view:agent-mission-control";

/// Full state rendered by Mission Control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionControlState {
    /// Missions shown in the operator view.
    pub missions: Vec<MissionCard>,
    /// Lease conflicts with exact targets.
    pub lease_conflicts: Vec<LeaseConflictCard>,
    /// Evidence events read from the Dev Cassette.
    pub evidence: Vec<EvidenceEvent>,
    /// Command intents exposed to the operator.
    pub intents: Vec<MissionControlIntent>,
}

/// One mission card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionCard {
    /// Mission id.
    pub id: Symbol,
    /// Mission goal.
    pub goal: String,
    /// Active agent roles.
    pub roles: Vec<String>,
    /// Recipe pattern label.
    pub recipe_pattern: String,
    /// Workspace leases.
    pub leases: Vec<LeaseClaim>,
    /// Validation status.
    pub validation: ValidationState,
    /// Human gates waiting or resolved for this mission.
    pub human_gates: Vec<HumanGate>,
    /// F6 explanation and attribution facets.
    pub facets: Vec<ExplanationFacet>,
}

/// A lease claim rendered in Mission Control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseClaim {
    /// Lease target kind: file, crate, or ide-object.
    pub target_kind: String,
    /// Exact target string.
    pub target: String,
    /// Lease mode.
    pub mode: String,
}

/// A visible lease conflict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseConflictCard {
    /// Left mission id.
    pub left_mission: Symbol,
    /// Left lease target.
    pub left: LeaseClaim,
    /// Right mission id.
    pub right_mission: Symbol,
    /// Right lease target.
    pub right: LeaseClaim,
}

/// Validation state for a mission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationState {
    /// Validation is not started.
    Pending,
    /// Validation is running.
    Running,
    /// Validation passed.
    Passed,
    /// Validation failed.
    Failed,
}

impl ValidationState {
    /// Stable status token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

/// Human gate shown in Mission Control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HumanGate {
    /// Gate id.
    pub id: String,
    /// Gate question or action.
    pub prompt: String,
    /// Gate status token.
    pub status: String,
}

/// F6 explanation or attribution facet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplanationFacet {
    /// Facet label.
    pub label: String,
    /// Evidence summary.
    pub evidence: String,
    /// Confidence token or percentage.
    pub confidence: String,
}

/// One replayable evidence event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceEvent {
    /// Sequence number in the cassette.
    pub sequence: u64,
    /// DevEnvelope kind.
    pub kind: Symbol,
    /// Text summary extracted from the payload when available.
    pub summary: String,
}

/// Command intent shown in Mission Control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionControlIntent {
    /// Intent kind, such as `approve` or `rerun-validation`.
    pub kind: &'static str,
    /// Button label.
    pub label: &'static str,
}

/// Renders Mission Control into a Scene value.
pub fn mission_control_view(state: &MissionControlState) -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("mission-control")),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![
                    mission_summary(state),
                    mission_list(&state.missions),
                    evidence_replay(&state.evidence),
                    lease_conflicts(&state.lease_conflicts),
                    command_bar(&state.intents),
                ]),
            ),
        ],
    )
}

/// Builds one replay frame per cassette event plus the initial frame.
pub fn mission_control_replay_frames(state: &MissionControlState) -> Vec<Expr> {
    (0..=state.evidence.len())
        .map(|count| {
            let mut frame = state.clone();
            frame.evidence.truncate(count);
            mission_control_view(&frame)
        })
        .collect()
}

/// Extracts evidence events directly from a Dev Cassette.
pub fn evidence_from_dev_cassette(cassette: &DevCassette) -> Vec<EvidenceEvent> {
    cassette
        .cassette()
        .envelopes()
        .iter()
        .enumerate()
        .filter_map(|(sequence, envelope)| {
            let StreamPacket::Data(packet) = envelope.packet() else {
                return None;
            };
            Some(EvidenceEvent {
                sequence: sequence as u64,
                kind: packet.kind.clone(),
                summary: payload_summary(&packet.payload)
                    .unwrap_or_else(|| packet.kind.name.to_string()),
            })
        })
        .collect()
}

/// The baseline Mission Control command intents.
pub fn mission_control_intents() -> Vec<MissionControlIntent> {
    vec![
        MissionControlIntent {
            kind: "approve",
            label: "Approve",
        },
        MissionControlIntent {
            kind: "reject",
            label: "Reject",
        },
        MissionControlIntent {
            kind: "ask",
            label: "Ask",
        },
        MissionControlIntent {
            kind: "split-mission",
            label: "Split",
        },
        MissionControlIntent {
            kind: "pause-agent",
            label: "Pause",
        },
        MissionControlIntent {
            kind: "rerun-validation",
            label: "Rerun validation",
        },
        MissionControlIntent {
            kind: "replay-cassette",
            label: "Replay",
        },
        MissionControlIntent {
            kind: "open-source",
            label: "Open source",
        },
    ]
}

fn mission_summary(state: &MissionControlState) -> Expr {
    node(
        "box",
        vec![
            ("role", sym("mission-summary")),
            (
                "children",
                Expr::List(vec![
                    text(format!("missions: {}", state.missions.len())),
                    text(format!("evidence events: {}", state.evidence.len())),
                    text(format!("lease conflicts: {}", state.lease_conflicts.len())),
                ]),
            ),
        ],
    )
}

fn mission_list(missions: &[MissionCard]) -> Expr {
    node(
        "grid",
        vec![
            ("role", sym("missions")),
            (
                "children",
                Expr::List(missions.iter().map(mission_card).collect()),
            ),
        ],
    )
}

fn mission_card(mission: &MissionCard) -> Expr {
    node(
        "box",
        vec![
            ("role", sym("mission-card")),
            ("mission", Expr::Symbol(mission.id.clone())),
            (
                "children",
                Expr::List(vec![
                    text(mission.goal.clone()),
                    data_line("recipe-pattern", &mission.recipe_pattern),
                    badge(mission.validation.token(), mission.validation.token()),
                    list_box("roles", mission.roles.iter().cloned()),
                    list_box("leases", mission.leases.iter().map(LeaseClaim::label)),
                    list_box(
                        "human-gates",
                        mission.human_gates.iter().map(HumanGate::label),
                    ),
                    list_box(
                        "explanation",
                        mission.facets.iter().map(ExplanationFacet::label),
                    ),
                ]),
            ),
        ],
    )
}

fn evidence_replay(evidence: &[EvidenceEvent]) -> Expr {
    let events = evidence
        .iter()
        .map(|event| {
            data_map(vec![
                ("at", uint(event.sequence)),
                ("event", Expr::Symbol(event.kind.clone())),
                ("label", Expr::String(event.summary.clone())),
            ])
        })
        .collect();
    node(
        "box",
        vec![
            ("role", sym("evidence-replay")),
            (
                "children",
                Expr::List(vec![
                    node(
                        "timeline",
                        vec![
                            ("lane", sym("dev-cassette")),
                            ("events", Expr::List(events)),
                        ],
                    ),
                    node(
                        "slider",
                        vec![
                            ("target", sym("replay-cassette")),
                            ("value", uint(evidence.len() as u64)),
                            ("max", uint(evidence.len() as u64)),
                        ],
                    ),
                ]),
            ),
        ],
    )
}

fn lease_conflicts(conflicts: &[LeaseConflictCard]) -> Expr {
    let rows = conflicts
        .iter()
        .map(|conflict| {
            node(
                "text",
                vec![(
                    "text",
                    Expr::String(format!(
                        "{} {} conflicts with {} {}",
                        conflict.left_mission,
                        conflict.left.label(),
                        conflict.right_mission,
                        conflict.right.label()
                    )),
                )],
            )
        })
        .collect();
    node(
        "box",
        vec![
            ("role", sym("lease-conflicts")),
            ("children", Expr::List(rows)),
        ],
    )
}

fn command_bar(intents: &[MissionControlIntent]) -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("mission-intents")),
            ("dir", sym("row")),
            (
                "children",
                Expr::List(
                    intents
                        .iter()
                        .map(|intent| {
                            node(
                                "button",
                                vec![
                                    (
                                        "intent",
                                        Expr::Symbol(Symbol::qualified("intent", intent.kind)),
                                    ),
                                    ("label", Expr::String(intent.label.to_owned())),
                                ],
                            )
                        })
                        .collect(),
                ),
            ),
        ],
    )
}

fn payload_summary(expr: &Expr) -> Option<String> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries.iter().find_map(|(key, value)| {
        let Expr::Symbol(symbol) = key else {
            return None;
        };
        if symbol.namespace.is_none() && symbol.name.as_ref() == "summary" {
            match value {
                Expr::String(summary) => Some(summary.clone()),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn data_line(label: &str, value: &str) -> Expr {
    text(format!("{label}: {value}"))
}

fn list_box(role: &str, items: impl Iterator<Item = String>) -> Expr {
    node(
        "box",
        vec![
            ("role", sym(role)),
            ("children", Expr::List(items.map(text).collect())),
        ],
    )
}

fn text(content: impl Into<String>) -> Expr {
    node("text", vec![("text", Expr::String(content.into()))])
}

fn badge(status: &str, label: &str) -> Expr {
    node(
        "badge",
        vec![
            ("status", sym(status)),
            ("label", Expr::String(label.to_owned())),
        ],
    )
}

impl LeaseClaim {
    fn label(&self) -> String {
        format!("{}:{} ({})", self.target_kind, self.target, self.mode)
    }
}

impl HumanGate {
    fn label(&self) -> String {
        format!("{} ({})", self.prompt, self.status)
    }
}

impl ExplanationFacet {
    fn label(&self) -> String {
        format!("{}: {} ({})", self.label, self.evidence, self.confidence)
    }
}
