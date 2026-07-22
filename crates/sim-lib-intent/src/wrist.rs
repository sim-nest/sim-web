//! Wrist raw-input reduction into ordinary Intent values.
//!
//! Watch hardware reports small physical events: buttons, touch, tap or raise
//! patterns, and sometimes a crown. This module keeps those inputs as local
//! pre-Intent data and reduces them to the same baseline Intent values used by
//! every other surface. Voice is intentionally absent here; speech arrives as an
//! Intent from an ASR model site.

use sim_kernel::{Expr, Symbol};
use sim_value::build;

use crate::gesture::Hit;
use crate::model::{Origin, intent};

/// Input capability lookup for a wrist profile.
///
/// Callers can pass a `DeviceProfile`'s `input` symbols without making this
/// crate depend on the device-profile crate.
pub trait WristInputCapabilities {
    /// Returns true when the profile advertises an input token.
    fn has_input(&self, name: &str) -> bool;
}

impl WristInputCapabilities for [Symbol] {
    fn has_input(&self, name: &str) -> bool {
        self.iter()
            .any(|symbol| symbol.namespace.is_none() && symbol.name.as_ref() == name)
    }
}

impl WristInputCapabilities for Vec<Symbol> {
    fn has_input(&self, name: &str) -> bool {
        self.as_slice().has_input(name)
    }
}

impl<T: WristInputCapabilities + ?Sized> WristInputCapabilities for &T {
    fn has_input(&self, name: &str) -> bool {
        (**self).has_input(name)
    }
}

/// Thresholds used while composing physical wrist input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WristInputTiming {
    /// Minimum spacing between accepted physical inputs.
    pub debounce_ms: u64,
    /// Maximum span for a single, double, or triple tap pattern.
    pub tap_sequence_ms: u64,
    /// Minimum button hold duration that becomes a dismiss intent.
    pub long_press_ms: u64,
    /// Minimum stable raise duration that becomes a selection intent.
    pub raise_stable_ms: u64,
}

impl Default for WristInputTiming {
    fn default() -> Self {
        Self {
            debounce_ms: 80,
            tap_sequence_ms: 450,
            long_press_ms: 650,
            raise_stable_ms: 180,
        }
    }
}

/// A physical watch input before it has Intent meaning.
#[derive(Clone, Debug, PartialEq)]
pub enum WristRawInput {
    /// A T-Rex button press.
    Button {
        /// Physical button id.
        id: Symbol,
        /// How long the button was held.
        held_ms: u64,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A crown or rotary input, only accepted when the profile advertises it.
    Crown {
        /// Rotation delta; zero with `press == false` is jitter.
        delta: i32,
        /// Whether the crown was pressed.
        press: bool,
        /// Current focus target.
        target: Expr,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A single, double, or triple tap pattern.
    Tap {
        /// Tap count in the recognized pattern.
        count: u8,
        /// Current focus target.
        target: Expr,
        /// Pattern span in milliseconds.
        span_ms: u64,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A raise pattern after motion filtering.
    Raise {
        /// Current focus target.
        target: Expr,
        /// Stable raise duration in milliseconds.
        stable_ms: u64,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A touch hit on the watch surface.
    Touch {
        /// Hit-test result on the current glance.
        hit: Hit,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
}

/// Stateful reducer for wrist input debouncing and Intent assignment.
#[derive(Clone, Debug, Default)]
pub struct WristIntentReducer {
    timing: WristInputTiming,
    last_accepted_ms: Option<u64>,
}

impl WristIntentReducer {
    /// Creates a reducer with default thresholds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a reducer with explicit thresholds.
    pub fn with_timing(timing: WristInputTiming) -> Self {
        Self {
            timing,
            last_accepted_ms: None,
        }
    }

    /// Reduces one wrist input to a standard Intent, or `None` for jitter,
    /// debounced repeats, unsupported inputs, and meaningless patterns.
    pub fn reduce<C: WristInputCapabilities + ?Sized>(
        &mut self,
        origin: Origin,
        raw: WristRawInput,
        profile: &C,
    ) -> Option<Expr> {
        let at_ms = raw.at_ms();
        let intent = match raw {
            WristRawInput::Button { id, held_ms, .. } if profile.has_input("button") => {
                if held_ms >= self.timing.long_press_ms {
                    Some(intent("dismiss", origin, vec![]))
                } else {
                    Some(invoke(
                        origin,
                        Expr::Symbol(id.clone()),
                        "button-press",
                        vec![Expr::Symbol(id)],
                    ))
                }
            }
            WristRawInput::Crown {
                delta,
                press,
                target,
                ..
            } if has_any_input(profile, &["crown", "rotary"]) => {
                if press {
                    Some(invoke(origin, target, "crown-press", vec![]))
                } else if delta != 0 {
                    Some(move_selection(origin, target, delta))
                } else {
                    None
                }
            }
            WristRawInput::Tap {
                count,
                target,
                span_ms,
                ..
            } if profile.has_input("tap") && span_ms <= self.timing.tap_sequence_ms => {
                tap_pattern(origin, count, target)
            }
            WristRawInput::Raise {
                target, stable_ms, ..
            } if profile.has_input("raise") && stable_ms >= self.timing.raise_stable_ms => {
                Some(select_one(origin, target))
            }
            WristRawInput::Touch { hit, .. } if profile.has_input("touch") => hit
                .target
                .clone()
                .map(|target| move_selection(origin, target, 1)),
            _ => None,
        }?;
        self.accepts(at_ms).then_some(intent)
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

impl WristRawInput {
    fn at_ms(&self) -> u64 {
        match self {
            WristRawInput::Button { at_ms, .. }
            | WristRawInput::Crown { at_ms, .. }
            | WristRawInput::Tap { at_ms, .. }
            | WristRawInput::Raise { at_ms, .. }
            | WristRawInput::Touch { at_ms, .. } => *at_ms,
        }
    }
}

fn has_any_input<C: WristInputCapabilities + ?Sized>(profile: &C, names: &[&str]) -> bool {
    names.iter().any(|name| profile.has_input(name))
}

fn tap_pattern(origin: Origin, count: u8, target: Expr) -> Option<Expr> {
    match count {
        1 => Some(select_one(origin, target)),
        2 => Some(invoke(origin, target, "double-tap", vec![])),
        3 => Some(intent(
            "edit",
            origin,
            vec![("target", target), ("path", Expr::List(Vec::new()))],
        )),
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
            ("op", Expr::Symbol(Symbol::qualified("watch/input", op))),
            ("args", Expr::List(args)),
        ],
    )
}

fn move_selection(origin: Origin, target: Expr, delta: i32) -> Expr {
    intent(
        "move",
        origin,
        vec![
            ("node", target),
            (
                "at",
                build::map(vec![("selection-delta", build::float(f64::from(delta)))]),
            ),
        ],
    )
}
