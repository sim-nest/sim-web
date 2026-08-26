//! Body-level attention arbitration shared by worn projections.

// conformance: attention tests prove quiet hours, coalescing, budgets, and manual continuation.

use std::collections::VecDeque;

/// A bounded prompt offered by a channel projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prompt {
    /// Stable coalescing key.
    pub key: String,
    /// Human-visible reduced summary.
    pub summary: String,
}

/// Explicit evidence explaining an attention decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttentionEvidence {
    /// Prompts combined behind this decision.
    pub coalesced: usize,
    /// Interruptions already spent in the current window.
    pub interruptions_spent: u32,
    /// Human-readable policy reason.
    pub reason: &'static str,
}

/// Normal outcomes of projecting a prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttentionDecision {
    /// Exactly one body-level prompt is visible.
    Present(Prompt, AttentionEvidence),
    /// Work remains available for manual continuation without interruption.
    ContinueManually(AttentionEvidence),
    /// Silence is the correct projection.
    Silent(AttentionEvidence),
}

/// Attention limits supplied by local user policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttentionPolicy {
    /// Inclusive start hour of quiet time.
    pub quiet_start_hour: u8,
    /// Exclusive end hour of quiet time.
    pub quiet_end_hour: u8,
    /// Maximum body interruptions in one policy window.
    pub max_interruptions: u32,
}

/// Stateful arbiter that admits at most one active body prompt.
#[derive(Debug)]
pub struct AttentionProjector {
    policy: AttentionPolicy,
    active: Option<Prompt>,
    pending: VecDeque<Prompt>,
    interruptions: u32,
}

impl AttentionProjector {
    /// Creates an empty projector.
    pub fn new(policy: AttentionPolicy) -> Self {
        Self {
            policy,
            active: None,
            pending: VecDeque::new(),
            interruptions: 0,
        }
    }

    /// Offers a reduced prompt; offline, dropped, and quiet channels stay silent.
    pub fn offer(&mut self, prompt: Prompt, hour: u8, online: bool) -> AttentionDecision {
        if !online {
            return self.silent("offline-or-dropped");
        }
        if self.is_quiet(hour) {
            self.coalesce(prompt);
            return self.silent("quiet-hours");
        }
        if self.interruptions >= self.policy.max_interruptions {
            self.coalesce(prompt);
            return AttentionDecision::ContinueManually(self.evidence("interruption-budget-spent"));
        }
        if self.active.is_some() {
            if self
                .active
                .as_ref()
                .is_some_and(|active| active.key == prompt.key)
            {
                self.active.as_mut().expect("active checked above").summary = prompt.summary;
            } else {
                self.coalesce(prompt);
            }
            return AttentionDecision::Present(
                self.active.clone().expect("active checked above"),
                self.evidence("one-active-prompt"),
            );
        }
        self.interruptions += 1;
        self.active = Some(prompt.clone());
        AttentionDecision::Present(prompt, self.evidence("admitted"))
    }

    /// Acknowledges the active prompt without automatically interrupting again.
    pub fn acknowledge(&mut self) {
        self.active = None;
    }

    fn coalesce(&mut self, prompt: Prompt) {
        if let Some(existing) = self.pending.iter_mut().find(|p| p.key == prompt.key) {
            existing.summary = prompt.summary;
        } else {
            self.pending.push_back(prompt);
        }
    }

    fn is_quiet(&self, hour: u8) -> bool {
        let start = self.policy.quiet_start_hour;
        let end = self.policy.quiet_end_hour;
        if start <= end {
            hour >= start && hour < end
        } else {
            hour >= start || hour < end
        }
    }

    fn evidence(&self, reason: &'static str) -> AttentionEvidence {
        AttentionEvidence {
            coalesced: self.pending.len(),
            interruptions_spent: self.interruptions,
            reason,
        }
    }

    fn silent(&self, reason: &'static str) -> AttentionDecision {
        AttentionDecision::Silent(self.evidence(reason))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projector(max: u32) -> AttentionProjector {
        AttentionProjector::new(AttentionPolicy {
            quiet_start_hour: 22,
            quiet_end_hour: 7,
            max_interruptions: max,
        })
    }

    #[test]
    fn burst_never_exceeds_one_body_level_prompt() {
        let mut p = projector(2);
        for n in 0..50 {
            let decision = p.offer(
                Prompt {
                    key: format!("job-{}", n % 3),
                    summary: format!("update-{n}"),
                },
                12,
                true,
            );
            let AttentionDecision::Present(_, evidence) = decision else {
                panic!("active prompt must remain visible")
            };
            assert!(evidence.interruptions_spent <= 1);
        }
    }

    #[test]
    fn silence_offline_quiet_and_manual_continuation_are_normal() {
        let prompt = Prompt {
            key: "mission".into(),
            summary: "ready".into(),
        };
        let mut p = projector(1);
        assert!(matches!(
            p.offer(prompt.clone(), 12, false),
            AttentionDecision::Silent(_)
        ));
        assert!(matches!(
            p.offer(prompt.clone(), 23, true),
            AttentionDecision::Silent(_)
        ));
        assert!(matches!(
            p.offer(prompt.clone(), 12, true),
            AttentionDecision::Present(_, _)
        ));
        p.acknowledge();
        assert!(matches!(
            p.offer(prompt, 12, true),
            AttentionDecision::ContinueManually(_)
        ));
    }
}
