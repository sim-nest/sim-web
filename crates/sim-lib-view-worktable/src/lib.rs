//! Rebuildable projection for a reversible expedition product.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    Modeled,
    Claimed,
    Stale,
    Unsupported,
    Verified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectState {
    Unavailable,
    Disarmed,
    Armed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomCard {
    pub pack: String,
    pub summary: String,
    pub route: Availability,
    pub device: Availability,
    pub fallback: String,
    pub effect: EffectState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueObservation {
    pub comparison: Option<String>,
    pub setup_minutes: u32,
    pub creative_block_minutes: u32,
    pub recoveries: u32,
    pub discarded_tools: Vec<String>,
    pub adopted_tools: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueConclusion {
    InsufficientEvidence,
    Compared { comparison: String },
}

impl ValueObservation {
    pub fn conclusion(&self) -> ValueConclusion {
        self.comparison
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .map_or(ValueConclusion::InsufficientEvidence, |comparison| {
                ValueConclusion::Compared {
                    comparison: comparison.clone(),
                }
            })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Worktable {
    pub expedition: Option<String>,
    pub pack_closure: Option<String>,
    pub rooms: BTreeMap<String, RoomCard>,
    pub evidence: Vec<String>,
    pub objections: Vec<String>,
    pub attention: Option<String>,
    pub export: Option<String>,
    pub edits: Vec<WorktableEdit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktableEdit {
    Open {
        expedition: String,
        pack_closure: String,
    },
    PutRoom(RoomCard),
    RemoveRoom {
        pack: String,
    },
    CiteEvidence(String),
    Object(String),
    Attend(String),
    Export(String),
}

impl Worktable {
    pub fn replay(edits: &[WorktableEdit]) -> Self {
        let mut table = Self::default();
        for edit in edits {
            table.apply(edit.clone());
        }
        table
    }

    pub fn apply(&mut self, edit: WorktableEdit) {
        match &edit {
            WorktableEdit::Open {
                expedition,
                pack_closure,
            } => {
                self.expedition = Some(expedition.clone());
                self.pack_closure = Some(pack_closure.clone());
            }
            WorktableEdit::PutRoom(card) => {
                self.rooms.insert(card.pack.clone(), card.clone());
            }
            WorktableEdit::RemoveRoom { pack } => {
                self.rooms.remove(pack);
            }
            WorktableEdit::CiteEvidence(value) => self.evidence.push(value.clone()),
            WorktableEdit::Object(value) => self.objections.push(value.clone()),
            WorktableEdit::Attend(value) => self.attention = Some(value.clone()),
            WorktableEdit::Export(value) => self.export = Some(value.clone()),
        }
        self.edits.push(edit);
    }

    pub fn undo(&mut self) -> Option<WorktableEdit> {
        let edit = self.edits.pop()?;
        *self = Self::replay(&self.edits);
        Some(edit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(pack: &str, effect: EffectState) -> RoomCard {
        RoomCard {
            pack: pack.into(),
            summary: "ready offline".into(),
            route: Availability::Modeled,
            device: Availability::Unsupported,
            fallback: "continue manually".into(),
            effect,
        }
    }

    #[test]
    fn projection_replays_undoes_and_exposes_effect_arming() {
        let edits = vec![
            WorktableEdit::Open {
                expedition: "content:expedition".into(),
                pack_closure: "sha256:closure".into(),
            },
            WorktableEdit::PutRoom(card("music-atlas", EffectState::Disarmed)),
            WorktableEdit::CiteEvidence("content:render".into()),
            WorktableEdit::Object("mapping-is-artistic".into()),
            WorktableEdit::Attend("manual-continuation".into()),
            WorktableEdit::Export("content:book-export".into()),
        ];
        let mut table = Worktable::replay(&edits);
        assert_eq!(table.edits, edits);
        assert_eq!(table.rooms["music-atlas"].effect, EffectState::Disarmed);
        assert_eq!(
            table.undo(),
            Some(WorktableEdit::Export("content:book-export".into()))
        );
        assert_eq!(table.export, None);
        assert_eq!(Worktable::replay(&table.edits), table);
    }

    #[test]
    fn value_projection_never_infers_personal_value() {
        let observation = ValueObservation {
            comparison: None,
            setup_minutes: 9,
            creative_block_minutes: 84,
            recoveries: 2,
            discarded_tools: vec!["candidate-a".into()],
            adopted_tools: vec!["candidate-b".into()],
        };
        assert_eq!(
            observation.conclusion(),
            ValueConclusion::InsufficientEvidence
        );
    }
}
