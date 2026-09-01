//! Calm, replayable phone interaction for continuity sessions.
//!
//! The journal is the only durable interaction state. [`PhoneController`]
//! rebuilds its projection from accepted continuity turns before every render
//! and transition; the Scene and audio gate are disposable derivatives.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use sim_kernel::{Cx, Expr, Result, Symbol};
use sim_lib_continuity::{
    ContinuityEvent, ContinuityJournal, ContinuityPlan, ContinuityState, JournalError,
    MemoryJournal, rebuild,
};
use sim_lib_intent::{Origin, field, intent};
use sim_lib_scene::{box_, node, stack, sym, text_node, validate_scene};
use sim_lib_view::{Draft, Operation, SurfaceCaps, SurfaceCodec};

/// Stable action names accepted by the phone controller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhoneAction {
    /// Begin capture after an explicit press.
    HoldToTalk,
    /// End capture at the button-release boundary.
    Release,
    /// Review the current transcript or result.
    Review,
    /// Submit the reviewed revision exactly once.
    Submit,
    /// Discard the current draft.
    Discard,
    /// Use typed input as the manual path.
    Type(String),
    /// Replay optional audio for the visible result.
    Replay,
    /// Defer the turn.
    Defer,
    /// Cancel the turn.
    Cancel,
    /// Stop every active or pending activity.
    Stop,
    /// Request stronger placement through ordinary realization.
    StrongerPlacement,
}

impl PhoneAction {
    fn name(&self) -> &'static str {
        match self {
            Self::HoldToTalk => "hold-to-talk",
            Self::Release => "release",
            Self::Review => "review",
            Self::Submit => "submit",
            Self::Discard => "discard",
            Self::Type(_) => "type",
            Self::Replay => "replay",
            Self::Defer => "defer",
            Self::Cancel => "cancel",
            Self::Stop => "stop",
            Self::StrongerPlacement => "stronger-placement",
        }
    }
}

/// External lifecycle signals. None can arm microphone capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassiveEvent {
    /// Browser focus changed.
    Focus,
    /// The page opened or resumed.
    PageOpen,
    /// A notification arrived.
    Notification,
    /// Connectivity changed.
    Connection,
    /// A stale callback arrived.
    Stale,
    /// Device layout rotated.
    Rotation,
}

/// Complete disposable phone projection rebuilt from the journal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhoneProjection {
    /// Next accepted continuity sequence, used as the visible revision.
    pub revision: u64,
    /// Current interaction state name.
    pub state: Symbol,
    /// Transcript text, usable without audio.
    pub transcript: Option<String>,
    /// Result text, usable without audio.
    pub result: Option<String>,
    /// Whether microphone capture is currently armed.
    pub capturing: bool,
    /// Whether optional sound is available.
    pub sound_available: bool,
    /// Whether this revision was already submitted.
    pub submitted: bool,
}

impl Default for PhoneProjection {
    fn default() -> Self {
        Self {
            revision: 0,
            state: Symbol::new("ready"),
            transcript: None,
            result: None,
            capturing: false,
            sound_available: false,
            submitted: false,
        }
    }
}

impl PhoneProjection {
    fn from_state(state: &ContinuityState) -> Self {
        let mut out = Self {
            revision: state.next_sequence,
            ..Self::default()
        };
        for turn in &state.turns {
            let event = &turn.event;
            match event.kind.name.as_ref() {
                "phone-capture-start" => {
                    out.state = Symbol::new("listening");
                    out.capturing = true;
                }
                "phone-release" => {
                    out.state = Symbol::new("transcribing");
                    out.capturing = false;
                }
                "phone-type" | "phone-transcript" => {
                    out.state = Symbol::new("transcript-review");
                    out.transcript = payload_text(&event.payload);
                    out.capturing = false;
                    out.submitted = false;
                }
                "phone-submit" => {
                    out.state = Symbol::new("submitted");
                    out.capturing = false;
                    out.submitted = true;
                }
                "phone-result" => {
                    out.state = Symbol::new("result-review");
                    out.result = payload_text(&event.payload);
                    out.sound_available = true;
                    out.capturing = false;
                }
                "phone-discard" => {
                    out = Self {
                        revision: turn.sequence + 1,
                        ..Self::default()
                    };
                }
                "phone-defer" => out.state = Symbol::new("deferred"),
                "phone-stop" => {
                    out.state = Symbol::new("stopped");
                    out.capturing = false;
                }
                "cancel" => {
                    out.state = Symbol::new("cancelled");
                    out.capturing = false;
                }
                _ => {}
            }
        }
        out.revision = state.next_sequence;
        out
    }

    /// Encodes this projection as the value consumed by [`PhoneSurfaceCodec`].
    pub fn to_expr(&self) -> Expr {
        sim_value::build::map(vec![
            ("revision", sim_value::build::uint(self.revision)),
            ("state", Expr::Symbol(self.state.clone())),
            ("transcript", option_text(&self.transcript)),
            ("result", option_text(&self.result)),
            ("capturing", Expr::Bool(self.capturing)),
            ("sound-available", Expr::Bool(self.sound_available)),
            ("submitted", Expr::Bool(self.submitted)),
        ])
    }
}

/// One generation-fenced optional PCM ingress.
#[derive(Clone, Debug, Default)]
struct AudioGate {
    generation: u64,
    accepting: bool,
    accepted_samples: usize,
}

impl AudioGate {
    fn start(&mut self) -> u64 {
        self.generation += 1;
        self.accepting = true;
        self.generation
    }
    fn stop(&mut self) {
        self.accepting = false;
        self.generation += 1;
    }
    fn accept(&mut self, generation: u64, pcm: &[i16]) -> usize {
        if !self.accepting || generation != self.generation {
            return 0;
        }
        self.accepted_samples += pcm.len();
        pcm.len()
    }
}

/// One controller over the continuity journal; it owns no parallel view state.
pub struct PhoneController {
    plan: ContinuityPlan,
    journal: MemoryJournal,
    audio: AudioGate,
    disclosure_ceiling: Vec<Symbol>,
}

impl PhoneController {
    /// Creates a controller with an explicit disclosure ceiling.
    pub fn new(plan: ContinuityPlan, disclosure_ceiling: Vec<Symbol>) -> Result<Self> {
        plan.validate()?;
        Ok(Self {
            plan,
            journal: MemoryJournal::default(),
            audio: AudioGate::default(),
            disclosure_ceiling,
        })
    }

    /// Rebuilds the complete disposable view from authoritative accepted turns.
    pub fn projection(&self) -> core::result::Result<PhoneProjection, JournalError> {
        rebuild(&self.plan, self.journal.turns())
            .map(|state| PhoneProjection::from_state(&state))
            .map_err(JournalError::Refused)
    }

    /// Returns the ordinary Intent corresponding to an explicit phone action.
    pub fn intent_for(&self, action: &PhoneAction, logical_time: u64) -> Expr {
        let fields = vec![
            (
                "target",
                Expr::Symbol(Symbol::qualified("surface", "continuity-phone")),
            ),
            ("op", Expr::Symbol(Symbol::new(action.name()))),
            (
                "args",
                match action {
                    PhoneAction::Type(text) => Expr::List(vec![Expr::String(text.clone())]),
                    _ => Expr::List(vec![]),
                },
            ),
        ];
        match action {
            PhoneAction::Cancel => intent(
                "cancel",
                Origin::human(logical_time),
                vec![("pane", Expr::String("continuity-phone".into()))],
            ),
            _ => intent("invoke", Origin::human(logical_time), fields),
        }
    }

    /// Applies an explicit action at an exact visible revision.
    pub fn apply_action(
        &mut self,
        action: PhoneAction,
        visible_revision: u64,
        logical_time: u64,
    ) -> core::result::Result<Expr, JournalError> {
        let state = rebuild(&self.plan, self.journal.turns()).map_err(JournalError::Refused)?;
        if visible_revision != state.next_sequence {
            return Err(JournalError::FenceConflict);
        }
        let before = PhoneProjection::from_state(&state);
        if action == PhoneAction::Submit && before.submitted {
            return Err(JournalError::FenceConflict);
        }
        if action == PhoneAction::HoldToTalk {
            self.audio.start();
        }
        if matches!(
            action,
            PhoneAction::Release
                | PhoneAction::Discard
                | PhoneAction::Defer
                | PhoneAction::Cancel
                | PhoneAction::Stop
        ) {
            self.audio.stop();
        }
        let ordinary = self.intent_for(&action, logical_time);
        let (kind, payload, disclosure) = event_parts(&action, &self.disclosure_ceiling);
        let event = ContinuityEvent {
            event_id: Symbol::qualified(
                "phone.event",
                format!("{}-{visible_revision}", action.name()),
            ),
            sequence: visible_revision,
            logical_time,
            kind: Symbol::new(kind),
            role: self
                .plan
                .roles
                .iter()
                .find(|role| role.root)
                .expect("validated plan has root")
                .role
                .clone(),
            lease: None,
            payload,
            disclosure,
        };
        self.journal.accept(&self.plan, &state, event)?;
        Ok(ordinary)
    }

    /// Accepts a transcript callback at its exact journal revision.
    pub fn accept_transcript(
        &mut self,
        text: String,
        visible_revision: u64,
        logical_time: u64,
    ) -> core::result::Result<(), JournalError> {
        self.accept_callback("phone-transcript", text, visible_revision, logical_time)
    }

    /// Accepts a result callback at its exact journal revision.
    pub fn accept_result(
        &mut self,
        text: String,
        visible_revision: u64,
        logical_time: u64,
    ) -> core::result::Result<(), JournalError> {
        self.accept_callback("phone-result", text, visible_revision, logical_time)
    }

    fn accept_callback(
        &mut self,
        kind: &str,
        text: String,
        visible_revision: u64,
        logical_time: u64,
    ) -> core::result::Result<(), JournalError> {
        let state = rebuild(&self.plan, self.journal.turns()).map_err(JournalError::Refused)?;
        if visible_revision != state.next_sequence {
            return Err(JournalError::FenceConflict);
        }
        self.audio.stop();
        let label = Symbol::new(if kind == "phone-result" {
            "result"
        } else {
            "transcript"
        });
        let (payload, disclosure) = if self.disclosure_ceiling.contains(&label) {
            (Expr::String(text), Some(label))
        } else {
            (Expr::String("[withheld]".into()), None)
        };
        let event = ContinuityEvent {
            event_id: Symbol::qualified("phone.callback", format!("{kind}-{visible_revision}")),
            sequence: visible_revision,
            logical_time,
            kind: Symbol::new(kind),
            role: self
                .plan
                .roles
                .iter()
                .find(|role| role.root)
                .expect("validated plan has root")
                .role
                .clone(),
            lease: None,
            payload,
            disclosure,
        };
        self.journal.accept(&self.plan, &state, event)?;
        Ok(())
    }

    /// Observes a passive lifecycle event without arming capture or mutating the journal.
    pub fn observe(&mut self, _event: PassiveEvent) {}

    /// Accepts PCM only for the live callback generation, returning the admitted sample count.
    pub fn accept_pcm(&mut self, callback_generation: u64, pcm: &[i16]) -> usize {
        self.audio.accept(callback_generation, pcm)
    }

    /// Returns the generation token for the currently armed callback, if any.
    pub fn capture_generation(&self) -> Option<u64> {
        self.audio.accepting.then_some(self.audio.generation)
    }
}

/// The single reversible codec for every phone continuity state and action.
#[derive(Default)]
pub struct PhoneSurfaceCodec;

impl SurfaceCodec for PhoneSurfaceCodec {
    fn encode(&self, _cx: &mut Cx, value: &Expr, caps: &SurfaceCaps) -> Result<Expr> {
        if caps.preset.name.as_ref() != "phone" {
            return Err(sim_kernel::Error::Eval(
                "continuity phone surface requires phone SurfaceCaps".into(),
            ));
        }
        let state = required_symbol(value, "state")?;
        let transcript = visible_text(value, "transcript");
        let result = visible_text(value, "result");
        let mut body = vec![
            sim_lib_scene::badge("current", state.name.as_ref()),
            text_node(transcript.as_deref().unwrap_or("Hold to talk or type")),
        ];
        if let Some(result) = result {
            body.push(text_node(result));
        }
        body.push(node(
            "button",
            vec![
                ("label", Expr::String("Stop".into())),
                ("action", sym("stop")),
            ],
        ));
        let scene = box_("continuity-phone", vec![stack("column", body)]);
        validate_scene(&scene)
            .map_err(|error| sim_kernel::Error::Eval(format!("invalid phone scene: {error}")))?;
        Ok(scene)
    }

    fn decode(&self, _cx: &mut Cx, value: &Expr, intent_value: &Expr) -> Result<Draft> {
        sim_lib_intent::validate_intent(intent_value)
            .map_err(|error| sim_kernel::Error::Eval(error.to_string()))?;
        let proposed = sim_value::build::map(vec![
            ("projection", value.clone()),
            ("intent", intent_value.clone()),
        ]);
        Ok(Draft::clean(value.clone(), proposed))
    }

    fn commit(&self, _cx: &mut Cx, draft: &Draft) -> Result<Operation> {
        if !draft.committable {
            return Err(sim_kernel::Error::Eval(
                "phone draft is not committable".into(),
            ));
        }
        Ok(Operation::new(draft.proposed.clone()))
    }
}

fn event_parts(action: &PhoneAction, ceiling: &[Symbol]) -> (&'static str, Expr, Option<Symbol>) {
    let (kind, payload, requested) = match action {
        PhoneAction::HoldToTalk => ("phone-capture-start", Expr::Nil, None),
        PhoneAction::Release => ("phone-release", Expr::Nil, None),
        PhoneAction::Review => ("phone-review", Expr::Nil, None),
        PhoneAction::Submit => ("phone-submit", Expr::Nil, None),
        PhoneAction::Discard => ("phone-discard", Expr::Nil, None),
        PhoneAction::Type(text) => (
            "phone-type",
            Expr::String(text.clone()),
            Some(Symbol::new("transcript")),
        ),
        PhoneAction::Replay => ("phone-replay", Expr::Nil, None),
        PhoneAction::Defer => ("phone-defer", Expr::Nil, None),
        PhoneAction::Cancel => ("cancel", Expr::Nil, None),
        PhoneAction::Stop => ("phone-stop", Expr::Nil, None),
        PhoneAction::StrongerPlacement => ("phone-stronger-placement", Expr::Nil, None),
    };
    match requested {
        Some(label) if ceiling.contains(&label) => (kind, payload, Some(label)),
        Some(_) => (kind, Expr::String("[withheld]".into()), None),
        None => (kind, payload, None),
    }
}

fn payload_text(value: &Expr) -> Option<String> {
    match value {
        Expr::String(text) => Some(text.clone()),
        _ => None,
    }
}
fn option_text(value: &Option<String>) -> Expr {
    value
        .as_ref()
        .map_or(Expr::Nil, |text| Expr::String(text.clone()))
}
fn visible_text(value: &Expr, name: &str) -> Option<String> {
    field(value, name).and_then(payload_text)
}
fn required_symbol(value: &Expr, name: &str) -> Result<Symbol> {
    match field(value, name) {
        Some(Expr::Symbol(symbol)) => Ok(symbol.clone()),
        _ => Err(sim_kernel::Error::Eval(format!(
            "phone projection requires symbol {name}"
        ))),
    }
}

#[cfg(test)]
mod tests;
