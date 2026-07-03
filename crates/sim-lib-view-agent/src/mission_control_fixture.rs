//! Deterministic Mission Control fixtures.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::data_map;
use sim_lib_stream_core::{DevCassette, DevEvent, LatencyClass};

use crate::mission_control::{
    ExplanationFacet, HumanGate, LeaseClaim, LeaseConflictCard, MissionCard, MissionControlState,
    ValidationState, evidence_from_dev_cassette, mission_control_intents,
};

/// Deterministic fake state for UI and cassette tests.
pub fn fake_mission_control_state() -> sim_kernel::Result<MissionControlState> {
    let cassette = fake_cassette()?;
    Ok(MissionControlState {
        missions: vec![MissionCard {
            id: Symbol::qualified("agent/mission", "mission-control-fixture"),
            goal: "Render Mission Control".to_owned(),
            roles: vec![
                "cartographer".to_owned(),
                "editor".to_owned(),
                "validator".to_owned(),
                "docs-agent".to_owned(),
                "human-gate".to_owned(),
            ],
            recipe_pattern: "a30-009-agentic-workflow".to_owned(),
            leases: vec![
                LeaseClaim {
                    target_kind: "crate".to_owned(),
                    target: "sim-lib-view-agent".to_owned(),
                    mode: "exclusive-write".to_owned(),
                },
                LeaseClaim {
                    target_kind: "file".to_owned(),
                    target: "crates/sim-lib-view-agent/src/mission_control.rs".to_owned(),
                    mode: "exclusive-write".to_owned(),
                },
                LeaseClaim {
                    target_kind: "ide-object".to_owned(),
                    target: "ide/object/agent-mission-control".to_owned(),
                    mode: "exclusive-write".to_owned(),
                },
            ],
            validation: ValidationState::Passed,
            human_gates: vec![HumanGate {
                id: "approve".to_owned(),
                prompt: "Approve Mission Control change".to_owned(),
                status: "waiting".to_owned(),
            }],
            facets: vec![ExplanationFacet {
                label: "F6 attribution".to_owned(),
                evidence: "Cassette evidence cites retrieval, guard, validation, and reflection"
                    .to_owned(),
                confidence: "0.91".to_owned(),
            }],
        }],
        lease_conflicts: vec![LeaseConflictCard {
            left_mission: Symbol::qualified("agent/mission", "mission-control-fixture"),
            left: LeaseClaim {
                target_kind: "crate".to_owned(),
                target: "sim-lib-view-agent".to_owned(),
                mode: "exclusive-write".to_owned(),
            },
            right_mission: Symbol::qualified("agent/mission", "docs-refresh"),
            right: LeaseClaim {
                target_kind: "crate".to_owned(),
                target: "sim-lib-view-agent".to_owned(),
                mode: "exclusive-write".to_owned(),
            },
        }],
        evidence: evidence_from_dev_cassette(&cassette),
        intents: mission_control_intents(),
    })
}

fn fake_cassette() -> sim_kernel::Result<DevCassette> {
    let node = Symbol::qualified("atelier/agent", "mission-control");
    DevCassette::from_events(
        Symbol::qualified("atelier/dev", "mission-control-fixture"),
        vec![
            DevEvent::new(
                "retrieval",
                node.clone(),
                LatencyClass::OfflineRender,
                summary("Ranked Mission Control context"),
            )?,
            DevEvent::new(
                "guard",
                node.clone(),
                LatencyClass::Interactive,
                summary("Lease accepted"),
            )?,
            DevEvent::validate(node.clone(), summary("sim-lib-view-agent tests passed"))?,
            DevEvent::new(
                "human-gate",
                node.clone(),
                LatencyClass::Interactive,
                summary("Approval waiting"),
            )?,
            DevEvent::new(
                "reflect",
                node,
                LatencyClass::OfflineRender,
                summary("F6 attribution is attached"),
            )?,
        ],
    )
}

fn summary(text: &str) -> Expr {
    data_map(vec![("summary", Expr::String(text.to_owned()))])
}
