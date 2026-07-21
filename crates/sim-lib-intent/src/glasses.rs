//! Glasses raw-input reduction into ordinary Intent values.
//!
//! Glasses report local physical input: gaze stability, head gestures, hand
//! rays, pinches, taps, buttons, and controller actions. This module keeps those
//! inputs as pre-Intent data and reduces them to the same baseline Intent values
//! used by every other surface. Speech is intentionally absent here; ASR sites
//! produce already-formed Intents.

use sim_kernel::{Expr, Symbol};
use sim_value::build;

use crate::gesture::Hit;
use crate::model::{Origin, intent};

/// Input capability lookup for a glasses profile.
///
/// Callers can pass a `DeviceProfile`'s `input` symbols without making this
/// crate depend on the device-profile crate.
pub trait GlassesInputCapabilities {
    /// Returns true when the profile advertises an input token.
    fn has_input(&self, name: &str) -> bool;
}

impl GlassesInputCapabilities for [Symbol] {
    fn has_input(&self, name: &str) -> bool {
        self.iter()
            .any(|symbol| symbol.namespace.is_none() && symbol.name.as_ref() == name)
    }
}

impl GlassesInputCapabilities for Vec<Symbol> {
    fn has_input(&self, name: &str) -> bool {
        self.as_slice().has_input(name)
    }
}

impl<T: GlassesInputCapabilities + ?Sized> GlassesInputCapabilities for &T {
    fn has_input(&self, name: &str) -> bool {
        (**self).has_input(name)
    }
}

/// Thresholds used while composing physical glasses input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlassesInputTiming {
    /// Minimum spacing between accepted physical inputs.
    pub debounce_ms: u64,
    /// Minimum stable gaze time for a focus move on enter or hold.
    pub gaze_stable_ms: u64,
    /// Minimum stable gaze time for a selection dwell.
    pub gaze_dwell_ms: u64,
    /// Minimum stable head-tracking time for head gestures.
    pub head_stable_ms: u64,
    /// Minimum stable hand-tracking time for rays and pinches.
    pub hand_stable_ms: u64,
    /// Maximum span for a single or double tap pattern.
    pub tap_sequence_ms: u64,
    /// Minimum held duration that turns a tap or button into edit/dismiss.
    pub long_press_ms: u64,
}

impl Default for GlassesInputTiming {
    fn default() -> Self {
        Self {
            debounce_ms: 80,
            gaze_stable_ms: 120,
            gaze_dwell_ms: 550,
            head_stable_ms: 140,
            hand_stable_ms: 80,
            tap_sequence_ms: 450,
            long_press_ms: 650,
        }
    }
}

/// Gaze phases after local tracking has identified a hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GazePhase {
    /// Gaze has entered a target and stayed there long enough to move focus.
    Enter,
    /// Gaze remains stable on a target but has not become a dwell selection.
    Hold,
    /// Gaze has stayed on a target long enough to select it.
    Dwell,
}

/// Stable head gestures reported by glasses tracking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadGesture {
    /// A nod, used as a direct invoke.
    Nod,
    /// A shake, used as dismiss.
    Shake,
    /// A left tilt, used as a focus move.
    TiltLeft,
    /// A right tilt, used as a focus move.
    TiltRight,
    /// An upward tilt, used as a focus move.
    TiltUp,
    /// A downward tilt, used as a focus move.
    TiltDown,
}

impl HeadGesture {
    fn token(self) -> &'static str {
        match self {
            HeadGesture::Nod => "head-nod",
            HeadGesture::Shake => "head-shake",
            HeadGesture::TiltLeft => "head-tilt-left",
            HeadGesture::TiltRight => "head-tilt-right",
            HeadGesture::TiltUp => "head-tilt-up",
            HeadGesture::TiltDown => "head-tilt-down",
        }
    }
}

/// Controller action reported by an accessory or glasses controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerAction {
    /// A primary press.
    Press,
    /// A directional move.
    Move {
        /// Horizontal step count.
        dx: i32,
        /// Vertical step count.
        dy: i32,
    },
    /// A request to edit the focused target.
    Edit,
}

/// A glasses input before it has Intent meaning.
#[derive(Clone, Debug, PartialEq)]
pub enum GlassesRawInput {
    /// A gaze phase over a hit target.
    Gaze {
        /// The recognized gaze phase.
        phase: GazePhase,
        /// Hit-test result under gaze.
        hit: Hit,
        /// How long the gaze has stayed on the hit.
        stable_ms: u64,
        /// Whether visual-inertial tracking is stable enough to trust the hit.
        vio_stable: bool,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A stable head gesture.
    Head {
        /// The recognized head gesture.
        kind: HeadGesture,
        /// Current focus target.
        target: Expr,
        /// How long the gesture has stayed stable.
        stable_ms: u64,
        /// Whether visual-inertial tracking is stable enough to trust the target.
        vio_stable: bool,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A hand ray over a hit target.
    HandRay {
        /// Hit-test result under the ray.
        hit: Hit,
        /// How long the ray has stayed stable.
        stable_ms: u64,
        /// Whether visual-inertial tracking is stable enough to trust the hit.
        vio_stable: bool,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A pinch over a hit target.
    Pinch {
        /// Hit-test result under the pinch.
        hit: Hit,
        /// How long the pinch has stayed stable.
        stable_ms: u64,
        /// Whether visual-inertial tracking is stable enough to trust the hit.
        vio_stable: bool,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A single, double, or long tap pattern.
    Tap {
        /// Tap count in the recognized pattern.
        count: u8,
        /// Current focus target.
        target: Expr,
        /// Pattern span in milliseconds.
        span_ms: u64,
        /// Duration of the final held tap.
        held_ms: u64,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A button on the glasses or an accessory.
    Button {
        /// Physical button id.
        id: Symbol,
        /// Current focus target, or `None` when the button is targetless.
        target: Option<Expr>,
        /// How long the button was held.
        held_ms: u64,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A controller or accessory action.
    Controller {
        /// Controller id.
        id: Symbol,
        /// Recognized controller action.
        action: ControllerAction,
        /// Current focus target.
        target: Expr,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
}

/// Stateful reducer for glasses input debouncing and Intent assignment.
#[derive(Clone, Debug, Default)]
pub struct GlassesIntentReducer {
    timing: GlassesInputTiming,
    last_accepted_ms: Option<u64>,
}

impl GlassesIntentReducer {
    /// Creates a reducer with default thresholds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a reducer with explicit thresholds.
    pub fn with_timing(timing: GlassesInputTiming) -> Self {
        Self {
            timing,
            last_accepted_ms: None,
        }
    }

    /// Reduces one glasses input to a standard Intent, or `None` for jitter,
    /// debounced repeats, unsupported inputs, and meaningless patterns.
    pub fn reduce<C: GlassesInputCapabilities + ?Sized>(
        &mut self,
        origin: Origin,
        raw: GlassesRawInput,
        profile: &C,
    ) -> Option<Expr> {
        let at_ms = raw.at_ms();
        let intent = match raw {
            GlassesRawInput::Gaze {
                phase,
                hit,
                stable_ms,
                vio_stable,
                ..
            } if profile.has_input("gaze") && vio_stable => {
                self.gaze_intent(origin, phase, hit, stable_ms)
            }
            GlassesRawInput::Head {
                kind,
                target,
                stable_ms,
                vio_stable,
                ..
            } if profile.has_input("head")
                && vio_stable
                && stable_ms >= self.timing.head_stable_ms =>
            {
                head_intent(origin, kind, target)
            }
            GlassesRawInput::HandRay {
                hit,
                stable_ms,
                vio_stable,
                ..
            } if has_any_input(profile, &["hand", "hand-ray"])
                && vio_stable
                && stable_ms >= self.timing.hand_stable_ms =>
            {
                hit.target
                    .clone()
                    .map(|target| move_focus(origin, target, "hand-ray"))
            }
            GlassesRawInput::Pinch {
                hit,
                stable_ms,
                vio_stable,
                ..
            } if has_any_input(profile, &["hand", "pinch"])
                && vio_stable
                && stable_ms >= self.timing.hand_stable_ms =>
            {
                hit.target
                    .clone()
                    .map(|target| invoke(origin, target, "pinch", vec![]))
            }
            GlassesRawInput::Tap {
                count,
                target,
                span_ms,
                held_ms,
                ..
            } if profile.has_input("tap") => {
                tap_intent(origin, count, target, span_ms, held_ms, self.timing)
            }
            GlassesRawInput::Button {
                id,
                target,
                held_ms,
                ..
            } if profile.has_input("button") => {
                if held_ms >= self.timing.long_press_ms {
                    Some(intent("dismiss", origin, vec![]))
                } else {
                    let target = target.unwrap_or_else(|| Expr::Symbol(id.clone()));
                    Some(invoke(
                        origin,
                        target,
                        "button-press",
                        vec![Expr::Symbol(id)],
                    ))
                }
            }
            GlassesRawInput::Controller {
                id, action, target, ..
            } if profile.has_input("controller") => controller_intent(origin, id, action, target),
            _ => None,
        }?;
        self.accepts(at_ms).then_some(intent)
    }

    fn gaze_intent(
        &self,
        origin: Origin,
        phase: GazePhase,
        hit: Hit,
        stable_ms: u64,
    ) -> Option<Expr> {
        let target = hit.target?;
        match phase {
            GazePhase::Dwell if stable_ms >= self.timing.gaze_dwell_ms => {
                Some(select_one(origin, target))
            }
            GazePhase::Enter | GazePhase::Hold if stable_ms >= self.timing.gaze_stable_ms => {
                Some(move_focus(origin, target, phase.token()))
            }
            _ => None,
        }
    }

    fn accepts(&mut self, at_ms: u64) -> bool {
        if let Some(last) = self.last_accepted_ms
            && at_ms.saturating_sub(last) < self.timing.debounce_ms
        {
            return false;
        }
        self.last_accepted_ms = Some(at_ms);
        true
    }
}

impl GazePhase {
    fn token(self) -> &'static str {
        match self {
            GazePhase::Enter => "gaze-enter",
            GazePhase::Hold => "gaze-hold",
            GazePhase::Dwell => "gaze-dwell",
        }
    }
}

impl GlassesRawInput {
    fn at_ms(&self) -> u64 {
        match self {
            GlassesRawInput::Gaze { at_ms, .. }
            | GlassesRawInput::Head { at_ms, .. }
            | GlassesRawInput::HandRay { at_ms, .. }
            | GlassesRawInput::Pinch { at_ms, .. }
            | GlassesRawInput::Tap { at_ms, .. }
            | GlassesRawInput::Button { at_ms, .. }
            | GlassesRawInput::Controller { at_ms, .. } => *at_ms,
        }
    }
}

fn has_any_input<C: GlassesInputCapabilities + ?Sized>(profile: &C, names: &[&str]) -> bool {
    names.iter().any(|name| profile.has_input(name))
}

fn head_intent(origin: Origin, kind: HeadGesture, target: Expr) -> Option<Expr> {
    match kind {
        HeadGesture::Nod => Some(invoke(origin, target, kind.token(), vec![])),
        HeadGesture::Shake => Some(intent("dismiss", origin, vec![])),
        HeadGesture::TiltLeft
        | HeadGesture::TiltRight
        | HeadGesture::TiltUp
        | HeadGesture::TiltDown => Some(move_focus(origin, target, kind.token())),
    }
}

fn tap_intent(
    origin: Origin,
    count: u8,
    target: Expr,
    span_ms: u64,
    held_ms: u64,
    timing: GlassesInputTiming,
) -> Option<Expr> {
    if count == 1 && held_ms >= timing.long_press_ms {
        return Some(edit(origin, target));
    }
    if span_ms > timing.tap_sequence_ms {
        return None;
    }
    match count {
        1 => Some(select_one(origin, target)),
        2 => Some(invoke(origin, target, "double-tap", vec![])),
        _ => None,
    }
}

fn controller_intent(
    origin: Origin,
    id: Symbol,
    action: ControllerAction,
    target: Expr,
) -> Option<Expr> {
    match action {
        ControllerAction::Press => Some(invoke(
            origin,
            target,
            "controller-press",
            vec![Expr::Symbol(id)],
        )),
        ControllerAction::Move { dx, dy } if dx != 0 || dy != 0 => {
            Some(move_by(origin, target, "controller-move", dx, dy))
        }
        ControllerAction::Edit => Some(edit(origin, target)),
        _ => None,
    }
}

fn select_one(origin: Origin, target: Expr) -> Expr {
    intent(
        "select",
        origin,
        vec![("targets", Expr::List(vec![target]))],
    )
}

fn invoke(origin: Origin, target: Expr, op: &str, args: Vec<Expr>) -> Expr {
    intent(
        "invoke",
        origin,
        vec![
            ("target", target),
            ("op", Expr::Symbol(Symbol::qualified("glasses/input", op))),
            ("args", Expr::List(args)),
        ],
    )
}

fn edit(origin: Origin, target: Expr) -> Expr {
    intent(
        "edit",
        origin,
        vec![("target", target), ("path", Expr::List(Vec::new()))],
    )
}

fn move_focus(origin: Origin, target: Expr, source: &str) -> Expr {
    intent(
        "move",
        origin,
        vec![
            ("node", target),
            ("at", build::map(vec![("source", build::sym(source))])),
        ],
    )
}

fn move_by(origin: Origin, target: Expr, source: &str, dx: i32, dy: i32) -> Expr {
    intent(
        "move",
        origin,
        vec![
            ("node", target),
            (
                "at",
                build::map(vec![
                    ("source", build::sym(source)),
                    ("dx", build::float(f64::from(dx))),
                    ("dy", build::float(f64::from(dy))),
                ]),
            ),
        ],
    )
}
