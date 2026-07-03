//! Change Capsule Scene for reviewable Atelier edits.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};
use sim_value::build::uint;

/// Change Capsule lens id.
pub const CHANGE_CAPSULE_LENS: &str = "view:agent-change-capsule";

/// Full state rendered by the Change Capsule view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeCapsuleViewState {
    /// Capsule id.
    pub id: Symbol,
    /// Diff summaries grouped by repository.
    pub diffs: Vec<CapsuleDiff>,
    /// Validation and docs logs.
    pub logs: Vec<CapsuleLog>,
    /// Generated-docs summary.
    pub generated_docs: Vec<GeneratedDocsSummary>,
    /// Planned pin updates.
    pub pin_plan: Vec<PinPlanView>,
    /// F6 fairness and attribution facet.
    pub fairness: CapsuleFairnessFacet,
    /// Replayable Dev Cassette summary.
    pub replay: CapsuleReplaySummary,
}

/// One diff row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapsuleDiff {
    /// Repository name.
    pub repo: String,
    /// Changed path.
    pub path: String,
    /// Diff summary.
    pub summary: String,
}

/// One validation or docs log row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapsuleLog {
    /// Log kind.
    pub kind: String,
    /// Command label.
    pub label: String,
    /// Outcome token.
    pub outcome: String,
    /// Evidence log path.
    pub log_path: String,
}

/// Generated-docs summary for one artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedDocsSummary {
    /// Repository name.
    pub repo: String,
    /// Generated path.
    pub path: String,
    /// Generator command.
    pub generator: String,
    /// Whether hand-edit policy is satisfied.
    pub regenerated: bool,
}

/// One pin update entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinPlanView {
    /// Repository name.
    pub repo: String,
    /// Current pinned commit.
    pub current_commit: String,
    /// New pinned commit.
    pub new_commit: String,
    /// Whether the new commit exists on the upstream remote before pinning.
    pub pushed_commit_exists: bool,
}

/// F6 fairness and attribution facet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapsuleFairnessFacet {
    /// Facet label.
    pub label: String,
    /// Evidence summary.
    pub evidence: String,
    /// Confidence token.
    pub confidence: String,
}

/// Dev Cassette replay summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapsuleReplaySummary {
    /// Recorded content hash.
    pub content_hash: String,
    /// Replay-computed content hash.
    pub replay_content_hash: String,
    /// Replay events.
    pub events: Vec<CapsuleReplayEvent>,
}

/// One replay event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapsuleReplayEvent {
    /// Sequence number.
    pub sequence: u64,
    /// Event kind.
    pub kind: String,
    /// Event summary.
    pub summary: String,
}

/// Renders a Change Capsule into a Scene value.
pub fn change_capsule_view(state: &ChangeCapsuleViewState) -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("change-capsule")),
            ("capsule", Expr::Symbol(state.id.clone())),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![
                    capsule_summary(state),
                    diff_panel(&state.diffs),
                    logs_panel(&state.logs),
                    generated_docs_panel(&state.generated_docs),
                    pin_plan_panel(&state.pin_plan),
                    fairness_panel(&state.fairness),
                    replay_panel(&state.replay),
                ]),
            ),
        ],
    )
}

/// Builds one replay frame per cassette event plus the initial frame.
pub fn change_capsule_replay_frames(state: &ChangeCapsuleViewState) -> Vec<Expr> {
    (0..=state.replay.events.len())
        .map(|count| {
            let mut frame = state.clone();
            frame.replay.events.truncate(count);
            change_capsule_view(&frame)
        })
        .collect()
}

/// Deterministic fake state for UI and replay tests.
pub fn fake_change_capsule_state() -> ChangeCapsuleViewState {
    ChangeCapsuleViewState {
        id: Symbol::qualified("atelier/capsule", "fixture"),
        diffs: vec![
            CapsuleDiff {
                repo: "sim-agent-net".to_owned(),
                path: "crates/sim-lib-agent/src/atelier/capsule.rs".to_owned(),
                summary: "agent-side capsule review model".to_owned(),
            },
            CapsuleDiff {
                repo: "sim-tooling".to_owned(),
                path: "src/atelier/capsule.rs".to_owned(),
                summary: "generated capsule cache".to_owned(),
            },
        ],
        logs: vec![
            CapsuleLog {
                kind: "validation".to_owned(),
                label: "agent-capsule-tests".to_owned(),
                outcome: "passed".to_owned(),
                log_path: ".sim/atelier/logs/agent-capsule-tests.log".to_owned(),
            },
            CapsuleLog {
                kind: "docs".to_owned(),
                label: "simdoc-check".to_owned(),
                outcome: "passed".to_owned(),
                log_path: ".sim/atelier/logs/simdoc-check.log".to_owned(),
            },
        ],
        generated_docs: vec![GeneratedDocsSummary {
            repo: "repo-docs".to_owned(),
            path: "docs/site/repos.md".to_owned(),
            generator: "simctl site".to_owned(),
            regenerated: true,
        }],
        pin_plan: vec![PinPlanView {
            repo: "sim-agent-net".to_owned(),
            current_commit: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            new_commit: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            pushed_commit_exists: true,
        }],
        fairness: CapsuleFairnessFacet {
            label: "F6 trade-off".to_owned(),
            evidence: "diff, validation, docs, pin, replay, risk, and rollback evidence".to_owned(),
            confidence: "0.92".to_owned(),
        },
        replay: CapsuleReplaySummary {
            content_hash: "fnv1a64:1111111111111111".to_owned(),
            replay_content_hash: "fnv1a64:1111111111111111".to_owned(),
            events: vec![
                CapsuleReplayEvent {
                    sequence: 0,
                    kind: "edit".to_owned(),
                    summary: "Patch capsule model".to_owned(),
                },
                CapsuleReplayEvent {
                    sequence: 1,
                    kind: "validate".to_owned(),
                    summary: "Validation passed".to_owned(),
                },
                CapsuleReplayEvent {
                    sequence: 2,
                    kind: "pin".to_owned(),
                    summary: "Pushed commit exists before pin".to_owned(),
                },
            ],
        },
    }
}

fn capsule_summary(state: &ChangeCapsuleViewState) -> Expr {
    node(
        "box",
        vec![
            ("role", sym("capsule-summary")),
            (
                "children",
                Expr::List(vec![
                    text(format!("diffs: {}", state.diffs.len())),
                    text(format!("logs: {}", state.logs.len())),
                    text(format!("pin updates: {}", state.pin_plan.len())),
                    text(format!("replay hash: {}", state.replay.replay_content_hash)),
                ]),
            ),
        ],
    )
}

fn diff_panel(diffs: &[CapsuleDiff]) -> Expr {
    panel(
        "capsule-diff",
        diffs
            .iter()
            .map(|diff| format!("{}:{} {}", diff.repo, diff.path, diff.summary)),
    )
}

fn logs_panel(logs: &[CapsuleLog]) -> Expr {
    panel(
        "capsule-logs",
        logs.iter().map(|log| {
            format!(
                "{} {} {} {}",
                log.kind, log.label, log.outcome, log.log_path
            )
        }),
    )
}

fn generated_docs_panel(docs: &[GeneratedDocsSummary]) -> Expr {
    panel(
        "generated-docs",
        docs.iter().map(|doc| {
            format!(
                "{}:{} {} regenerated={}",
                doc.repo, doc.path, doc.generator, doc.regenerated
            )
        }),
    )
}

fn pin_plan_panel(pins: &[PinPlanView]) -> Expr {
    let rows = pins
        .iter()
        .map(|pin| {
            data_map(vec![
                ("repo", Expr::String(pin.repo.clone())),
                ("current", Expr::String(pin.current_commit.clone())),
                ("new", Expr::String(pin.new_commit.clone())),
                ("pushed", Expr::Bool(pin.pushed_commit_exists)),
            ])
        })
        .collect();
    node(
        "box",
        vec![
            ("role", sym("pin-plan")),
            (
                "children",
                Expr::List(vec![node("table", vec![("rows", Expr::List(rows))])]),
            ),
        ],
    )
}

fn fairness_panel(facet: &CapsuleFairnessFacet) -> Expr {
    node(
        "box",
        vec![
            ("role", sym("fairness-facet")),
            (
                "children",
                Expr::List(vec![
                    text(facet.label.clone()),
                    text(facet.evidence.clone()),
                    text(format!("confidence: {}", facet.confidence)),
                ]),
            ),
        ],
    )
}

fn replay_panel(replay: &CapsuleReplaySummary) -> Expr {
    let events = replay
        .events
        .iter()
        .map(|event| {
            data_map(vec![
                ("at", uint(event.sequence)),
                (
                    "event",
                    Expr::Symbol(Symbol::qualified("ide/event", event.kind.as_str())),
                ),
                ("label", Expr::String(event.summary.clone())),
            ])
        })
        .collect();
    node(
        "box",
        vec![
            ("role", sym("replay-cassette")),
            (
                "children",
                Expr::List(vec![
                    text(format!("content hash: {}", replay.content_hash)),
                    text(format!("replay hash: {}", replay.replay_content_hash)),
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
                            ("value", uint(replay.events.len() as u64)),
                            ("max", uint(replay.events.len() as u64)),
                        ],
                    ),
                ]),
            ),
        ],
    )
}

fn panel(role: &str, rows: impl Iterator<Item = String>) -> Expr {
    node(
        "box",
        vec![
            ("role", sym(role)),
            ("children", Expr::List(rows.map(text).collect())),
        ],
    )
}

fn text(value: String) -> Expr {
    node("text", vec![("text", Expr::String(value))])
}
