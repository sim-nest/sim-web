//! Experience modes and capability-aware action exposure.
//!
//! One runtime serves five audiences without forking into five products.
//! Audience differences are expressed as **modes** -- Household, Builder,
//! Systems -- that change which lenses, controls, and verbosity are shown, gated
//! by capability and role. Modes never change the underlying value: the same
//! value renders at different depth, but it is the same value.

use sim_kernel::{CapabilityName, Expr, Symbol};
use sim_lib_scene::node;

use crate::universal_view::universal_regions;

/// An experience mode. Modes change depth and control exposure, not the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Large, safe, jargon-free controls for non-technical users.
    Household,
    /// The default working depth for regular users and coders.
    Builder,
    /// Full depth -- structure, operations, raw -- for developers and admins.
    Systems,
}

impl Mode {
    /// Parse a mode symbol name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "household" => Some(Mode::Household),
            "builder" => Some(Mode::Builder),
            "systems" => Some(Mode::Systems),
            _ => None,
        }
    }

    /// The mode's symbol.
    pub fn symbol(self) -> Symbol {
        Symbol::new(match self {
            Mode::Household => "household",
            Mode::Builder => "builder",
            Mode::Systems => "systems",
        })
    }

    /// How many universal regions this mode shows (its depth).
    pub fn depth(self) -> usize {
        match self {
            Mode::Household => 2,
            Mode::Builder => 3,
            Mode::Systems => 4,
        }
    }
}

/// Render a value through the universal default lens at the depth of `mode`.
/// Household shows a friendly summary and the canonical text; Builder adds the
/// structure tree; Systems adds the operations inspector. The value is never
/// changed.
pub fn universal_scene(value: &Expr, mode: Mode) -> Expr {
    let mut regions = universal_regions(value);
    regions.truncate(mode.depth());
    node(
        "stack",
        vec![
            ("id", Expr::Symbol(Symbol::new("universal"))),
            ("dir", Expr::Symbol(Symbol::new("column"))),
            ("mode", Expr::Symbol(mode.symbol())),
            ("children", Expr::List(regions)),
        ],
    )
}

/// Whether and how an action is exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exposure {
    /// Shown and directly actionable.
    Shown,
    /// Shown but requires a confirmation overlay carrying the exact operation.
    ConfirmationGated,
    /// Absent entirely (not disabled-and-tantalizing).
    Absent,
}

/// Decide how to expose an action, given the capabilities it requires, the
/// granted set, whether it is dangerous, and the active mode.
///
/// A missing capability makes the action absent (admin actions do not appear
/// disabled). A dangerous action is confirmation-gated when capable, and absent
/// in Household mode. Everything else is shown.
pub fn action_exposure(
    required: &[CapabilityName],
    granted: impl Fn(&CapabilityName) -> bool,
    dangerous: bool,
    mode: Mode,
) -> Exposure {
    if !required.iter().all(&granted) {
        return Exposure::Absent;
    }
    if dangerous {
        return match mode {
            Mode::Household => Exposure::Absent,
            _ => Exposure::ConfirmationGated,
        };
    }
    Exposure::Shown
}

/// A clear "action denied" Scene -- never a blank dead end.
pub fn denied_scene(reason: &str) -> Expr {
    node(
        "box",
        vec![
            ("role", Expr::Symbol(Symbol::new("denied"))),
            (
                "children",
                Expr::List(vec![
                    node(
                        "badge",
                        vec![
                            ("status", Expr::Symbol(Symbol::new("error"))),
                            ("label", Expr::String("denied".to_owned())),
                        ],
                    ),
                    node("text", vec![("text", Expr::String(reason.to_owned()))]),
                ]),
            ),
        ],
    )
}

/// A read-only rendering: the value at the mode's depth, clearly marked
/// read-only, with no committing controls.
pub fn readonly_scene(value: &Expr, mode: Mode) -> Expr {
    node(
        "stack",
        vec![
            ("role", Expr::Symbol(Symbol::new("readonly"))),
            ("dir", Expr::Symbol(Symbol::new("column"))),
            (
                "children",
                Expr::List(vec![
                    node(
                        "badge",
                        vec![
                            ("status", Expr::Symbol(Symbol::new("info"))),
                            ("label", Expr::String("read-only".to_owned())),
                        ],
                    ),
                    universal_scene(value, mode),
                ]),
            ),
        ],
    )
}
