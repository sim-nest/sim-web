//! The gesture algebra: composing raw browser gestures into one Intent value.
//!
//! Raw input handling (pointer down/move/up streams, key events, paste) stays
//! in the browser. This crate owns the *meaning*: a small algebra that folds a
//! recognized raw gesture plus the thing under the pointer into a single
//! checked Intent value. The browser feeds pointer events into a
//! [`GestureRecognizer`], which yields a [`RawGesture`]; [`intent_from_gesture`]
//! turns that into an Intent (carrying the operator and tick), or returns a
//! diagnostic when the gesture has no meaning in context.

use sim_kernel::Expr;

use crate::model::{IntentError, Origin, intent};

/// What sits under the pointer at the moment of a gesture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HitRole {
    /// Empty workspace background.
    Blank,
    /// A graph node body.
    Node,
    /// A typed port on a node.
    Port,
    /// An actionable control.
    Button,
    /// An editable field.
    Field,
    /// A wire between two ports.
    Edge,
}

/// A hit-test result: what was under the pointer, the runtime value it stands
/// for, and any role-specific detail (for example a port's `node`/`port`/`dir`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The kind of element hit.
    pub role: HitRole,
    /// The runtime value the element represents, if any.
    pub target: Option<Expr>,
    /// Role-specific fields (control name, port descriptor, field path, ...).
    pub detail: Vec<(String, Expr)>,
}

impl Hit {
    /// A blank-background hit.
    pub fn blank() -> Self {
        Self {
            role: HitRole::Blank,
            target: None,
            detail: Vec::new(),
        }
    }

    /// A hit on `role` standing for `target`.
    pub fn on(role: HitRole, target: Expr) -> Self {
        Self {
            role,
            target: Some(target),
            detail: Vec::new(),
        }
    }

    /// Attach a role-specific detail field.
    pub fn with(mut self, key: &str, value: Expr) -> Self {
        self.detail.push((key.to_owned(), value));
        self
    }

    fn detail(&self, key: &str) -> Option<&Expr> {
        self.detail
            .iter()
            .find_map(|(name, value)| (name == key).then_some(value))
    }
}

/// One pointer event in a raw input stream.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerEvent {
    /// The phase of this event.
    pub phase: PointerPhase,
    /// Pointer x position in workspace coordinates.
    pub x: f64,
    /// Pointer y position in workspace coordinates.
    pub y: f64,
    /// What is under the pointer for this event.
    pub hit: Hit,
}

/// The phase of a pointer event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    /// Pointer pressed.
    Down,
    /// Pointer moved while pressed.
    Move,
    /// Pointer released.
    Up,
}

/// A recognized raw gesture: the largest unit the browser composes before
/// meaning is assigned.
#[derive(Clone, Debug, PartialEq)]
pub enum RawGesture {
    /// A press and release on the same element with no significant drag.
    Tap {
        /// Element the tap landed on.
        hit: Hit,
    },
    /// A press, drag, and release ending on `to`, with the final position.
    Drag {
        /// Element the drag started on.
        from: Hit,
        /// Element the drag ended on.
        to: Hit,
        /// Final pointer position as `(x, y)`.
        at: (f64, f64),
    },
    /// A keyboard command directed at `hit` (for example delete/commit/cancel).
    Key {
        /// Command name carried by the key gesture.
        command: String,
        /// Element the command is directed at.
        hit: Hit,
    },
}

/// Folds a pointer-event stream into [`RawGesture`]s. The browser pushes each
/// pointer event; a complete gesture is returned on the release event.
#[derive(Debug, Default)]
pub struct GestureRecognizer {
    down: Option<(Hit, f64, f64)>,
    moved: bool,
    last: (f64, f64),
}

impl GestureRecognizer {
    /// Create a recognizer with no gesture in progress.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one pointer event; returns a [`RawGesture`] when one completes.
    pub fn pointer(&mut self, event: PointerEvent) -> Option<RawGesture> {
        match event.phase {
            PointerPhase::Down => {
                self.down = Some((event.hit, event.x, event.y));
                self.moved = false;
                self.last = (event.x, event.y);
                None
            }
            PointerPhase::Move => {
                if let Some((_, start_x, start_y)) = &self.down {
                    if (event.x - start_x).abs() > DRAG_THRESHOLD
                        || (event.y - start_y).abs() > DRAG_THRESHOLD
                    {
                        self.moved = true;
                    }
                    self.last = (event.x, event.y);
                }
                None
            }
            PointerPhase::Up => {
                let (from, _, _) = self.down.take()?;
                let at = (event.x, event.y);
                if self.moved {
                    Some(RawGesture::Drag {
                        from,
                        to: event.hit,
                        at,
                    })
                } else {
                    Some(RawGesture::Tap { hit: from })
                }
            }
        }
    }

    /// Compose a keyboard command into a raw gesture directed at `hit`.
    pub fn key(command: &str, hit: Hit) -> RawGesture {
        RawGesture::Key {
            command: command.to_owned(),
            hit,
        }
    }
}

const DRAG_THRESHOLD: f64 = 3.0;

/// Turn a recognized raw gesture into an Intent value in `pane`, attributed to
/// `operator` at `tick`. Returns a diagnostic when the gesture is meaningless
/// in context; nothing mutates on failure.
pub fn intent_from_gesture(
    operator: Origin,
    pane: &str,
    raw: &RawGesture,
) -> Result<Expr, IntentError> {
    match raw {
        RawGesture::Tap { hit } => tap_intent(operator, hit),
        RawGesture::Drag { from, to, at } => drag_intent(operator, from, to, *at),
        RawGesture::Key { command, hit } => key_intent(operator, pane, command, hit),
    }
}

fn ungesturable(message: &str) -> IntentError {
    IntentError {
        path: vec!["gesture".to_owned()],
        message: message.to_owned(),
    }
}

fn pane_field(pane: &str) -> Expr {
    sim_value::build::sym(pane)
}

fn at_field(at: (f64, f64)) -> Expr {
    sim_value::build::map(vec![
        ("x", sim_value::build::float(at.0)),
        ("y", sim_value::build::float(at.1)),
    ])
}

fn require_target(hit: &Hit, message: &str) -> Result<Expr, IntentError> {
    hit.target.clone().ok_or_else(|| ungesturable(message))
}

fn tap_intent(operator: Origin, hit: &Hit) -> Result<Expr, IntentError> {
    match hit.role {
        HitRole::Button => {
            let target = require_target(hit, "tap on a control with no target")?;
            let control = hit
                .detail("control")
                .cloned()
                .ok_or_else(|| ungesturable("button hit is missing a 'control'"))?;
            Ok(intent(
                "tap",
                operator,
                vec![("target", target), ("control", control)],
            ))
        }
        HitRole::Node | HitRole::Port | HitRole::Edge | HitRole::Field => {
            let target = require_target(hit, "selectable hit with no target")?;
            Ok(intent(
                "select",
                operator,
                vec![("targets", Expr::List(vec![target]))],
            ))
        }
        HitRole::Blank => Ok(intent(
            "select",
            operator,
            vec![("targets", Expr::List(vec![]))],
        )),
    }
}

fn drag_intent(
    operator: Origin,
    from: &Hit,
    to: &Hit,
    at: (f64, f64),
) -> Result<Expr, IntentError> {
    match (&from.role, &to.role) {
        (HitRole::Port, HitRole::Port) => {
            let from_port = port_descriptor(from)?;
            let to_port = port_descriptor(to)?;
            Ok(intent(
                "wire",
                operator,
                vec![("from", from_port), ("to", to_port)],
            ))
        }
        (HitRole::Node, _) => {
            let node = require_target(from, "drag of a node with no target")?;
            Ok(intent(
                "move",
                operator,
                vec![("node", node), ("at", at_field(at))],
            ))
        }
        _ => Err(ungesturable("drag has no meaning between these elements")),
    }
}

fn port_descriptor(hit: &Hit) -> Result<Expr, IntentError> {
    let node = hit
        .detail("node")
        .cloned()
        .ok_or_else(|| ungesturable("port hit is missing a 'node'"))?;
    let port = hit
        .detail("port")
        .cloned()
        .ok_or_else(|| ungesturable("port hit is missing a 'port'"))?;
    Ok(sim_value::build::map(vec![("node", node), ("port", port)]))
}

fn key_intent(operator: Origin, pane: &str, command: &str, hit: &Hit) -> Result<Expr, IntentError> {
    match command {
        "delete" => {
            let target = require_target(hit, "delete with no target under the pointer")?;
            Ok(intent(
                "delete",
                operator,
                vec![("targets", Expr::List(vec![target]))],
            ))
        }
        "commit" => Ok(intent("commit", operator, vec![("pane", pane_field(pane))])),
        "cancel" => Ok(intent("cancel", operator, vec![("pane", pane_field(pane))])),
        other => Err(ungesturable(&format!(
            "no Intent bound to command '{other}'"
        ))),
    }
}
