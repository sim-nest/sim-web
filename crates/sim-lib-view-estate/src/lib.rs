#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Reversible estate Surface scenes. Rendering is pure; operations compile to
//! exact typed organ calls and never carry shell or provider command text.

use sim_kernel::Expr;
use sim_lib_estate_book::Key;
use sim_lib_estate_serve::Call;
use sim_lib_scene::{data_map, node, sym};

/// Every estate scene supported by the shared Surface protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneKind {
    /// Cross-estate summary.
    Overview,
    /// Provider and inventory discovery.
    Discovery,
    /// Content-bound proposed change.
    PlanDiff,
    /// Human approval review.
    Review,
    /// Live durable event stream.
    LiveEvents,
    /// Verification result.
    Verification,
    /// Historical runs.
    History,
    /// Ambiguous post-dispatch state.
    Unknown,
    /// Isolated targets and evidence.
    Quarantine,
    /// Reconciliation result.
    Reconciliation,
}

/// Complete visible review material, content-bound to one plan key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Review {
    /// Exact content-addressed plan.
    pub plan: Key,
    /// Sanitized provider identity.
    pub provider: String,
    /// Portable project fingerprint.
    pub project: String,
    /// Content-addressed inventory.
    pub inventory: Key,
    /// Exact sanitized targets.
    pub targets: Vec<String>,
    /// Shaped parameter display.
    pub parameters: Vec<(String, String)>,
    /// Computed risk class.
    pub risk: String,
    /// Absolute approval expiry.
    pub expires_at: u64,
    /// Provider preview evidence.
    pub preview: String,
    /// Required verification policy.
    pub verification: String,
}

/// Reversible user operations recognized by the estate lens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Edit {
    /// Construct a plan for a closed operation id.
    Plan {
        /// Closed exposure operation id.
        operation: String,
    },
    /// Request human review of the visible plan.
    Review,
    /// Apply with exact reviewed evidence.
    Apply {
        /// Content-bound approval evidence.
        approval: Key,
    },
    /// Reconcile a run from retained evidence.
    Reconcile {
        /// Durable run id.
        run: String,
    },
    /// Apply an explicitly reviewed quarantine override.
    Override {
        /// Quarantined run id.
        run: String,
        /// Content-bound override approval.
        approval: Key,
    },
}

/// Stale-scene refusal from reverse compilation.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("stale estate scene: expected {expected:?}, current {current:?}")]
pub struct Stale {
    /// Plan key captured by the scene.
    pub expected: Key,
    /// Current organ plan key.
    pub current: Key,
}

/// One codec object owns both the forward scene and reverse operation paths.
pub struct EstateSurfaceCodec;
impl EstateSurfaceCodec {
    /// Render any estate state into a valid Scene value.
    pub fn encode(kind: SceneKind, plan: Option<&Key>, fields: Vec<(&str, Expr)>) -> Expr {
        let mut data = vec![
            ("lens", sym("view:estate")),
            ("estate-scene", sym(scene_name(kind))),
        ];
        if let Some(key) = plan {
            data.push(("plan-key", Expr::String(key.0.clone())));
        }
        data.extend(fields);
        node(
            if kind == SceneKind::LiveEvents {
                "timeline"
            } else {
                "table"
            },
            data,
        )
    }

    /// Render review with every authority-relevant field visible before issuance.
    pub fn review(review: &Review) -> Expr {
        Self::encode(
            SceneKind::Review,
            Some(&review.plan),
            vec![(
                "review",
                data_map(vec![
                    ("provider", Expr::String(review.provider.clone())),
                    ("project", Expr::String(review.project.clone())),
                    ("inventory", Expr::String(review.inventory.0.clone())),
                    (
                        "targets",
                        Expr::List(review.targets.iter().cloned().map(Expr::String).collect()),
                    ),
                    (
                        "parameters",
                        Expr::List(
                            review
                                .parameters
                                .iter()
                                .map(|(k, v)| {
                                    data_map(vec![
                                        ("name", Expr::String(k.clone())),
                                        ("value", Expr::String(v.clone())),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                    ("risk", Expr::String(review.risk.clone())),
                    ("expiry", Expr::String(review.expires_at.to_string())),
                    ("preview", Expr::String(review.preview.clone())),
                    ("verification", Expr::String(review.verification.clone())),
                ]),
            )],
        )
    }

    /// Compile an edit to the exact current plan, refusing stale scenes.
    pub fn decode(scene_plan: &Key, current_plan: &Key, edit: Edit) -> Result<Call, Stale> {
        if scene_plan != current_plan {
            return Err(Stale {
                expected: scene_plan.clone(),
                current: current_plan.clone(),
            });
        }
        Ok(match edit {
            Edit::Plan { operation } => Call::Plan { operation },
            Edit::Review => Call::Review {
                plan: current_plan.clone(),
            },
            Edit::Apply { approval } => Call::Apply {
                plan: current_plan.clone(),
                approval,
            },
            Edit::Reconcile { run } => Call::Reconcile { run },
            Edit::Override { run, approval } => Call::Override {
                run,
                plan: current_plan.clone(),
                approval,
            },
        })
    }
}

fn scene_name(kind: SceneKind) -> &'static str {
    match kind {
        SceneKind::Overview => "overview",
        SceneKind::Discovery => "discovery",
        SceneKind::PlanDiff => "plan-diff",
        SceneKind::Review => "review",
        SceneKind::LiveEvents => "live-events",
        SceneKind::Verification => "verification",
        SceneKind::History => "history",
        SceneKind::Unknown => "unknown",
        SceneKind::Quarantine => "quarantine",
        SceneKind::Reconciliation => "reconciliation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_scenes_are_valid() {
        for kind in [
            SceneKind::Overview,
            SceneKind::Discovery,
            SceneKind::PlanDiff,
            SceneKind::Review,
            SceneKind::LiveEvents,
            SceneKind::Verification,
            SceneKind::History,
            SceneKind::Unknown,
            SceneKind::Quarantine,
            SceneKind::Reconciliation,
        ] {
            assert!(
                sim_lib_scene::validate_scene(&EstateSurfaceCodec::encode(kind, None, vec![]))
                    .is_ok()
            );
        }
    }
    #[test]
    fn stale_operation_refuses() {
        let old = Key("old".into());
        let new = Key("new".into());
        assert!(EstateSurfaceCodec::decode(&old, &new, Edit::Review).is_err());
    }
    #[test]
    fn operation_contains_no_command_text() {
        let key = Key("plan".into());
        let call = EstateSurfaceCodec::decode(
            &key,
            &key,
            Edit::Apply {
                approval: Key("approval".into()),
            },
        )
        .unwrap();
        assert_eq!(
            call,
            Call::Apply {
                plan: key,
                approval: Key("approval".into())
            }
        );
    }
}
