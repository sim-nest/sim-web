//! Scene node kinds.
//!
//! Scene node kinds are open metadata, never a closed kernel enum: a scene node
//! is an `Expr::Map` carrying a `kind` entry whose value is a symbol in the
//! `scene` namespace (for example `scene/graph`). This module lists the minimum
//! baseline scene vocabulary and provides recognition helpers. New kinds can be
//! added by libs without touching the kernel; the recognized set here is the
//! baseline the universal lenses rely on.

use sim_kernel::Symbol;

/// The namespace every scene node `kind` symbol lives in.
pub const SCENE_NAMESPACE: &str = "scene";

/// The map key that tags a scene node with its kind.
pub const KIND_KEY: &str = "kind";

/// The baseline scene node kind names (the local part of the `scene/*` symbol).
///
/// The recognized baseline, not a closed universe: [`is_known_kind`] returns
/// `false` for an unrecognized kind so a malformed scene fails closed, while
/// libs may extend the runtime set through registration.
pub const SCENE_KINDS: &[&str] = &[
    "box",
    "stack",
    "grid",
    "text",
    "field",
    "button",
    "badge",
    "badge-cluster",
    "icon",
    "tree",
    "table",
    "graph",
    "glance",
    "spatial",
    "anchor",
    "panel",
    "gaze-cursor",
    "hand-ray",
    "world-plane",
    "node",
    "edge",
    "plot",
    "matrix",
    "knob",
    "slider",
    "meter",
    "waveform",
    "spectrum",
    "timeline",
    "keyboard",
    "piano-roll",
    "player-rack",
    "object-roll",
    "canvas",
    "overlay",
    "embed",
    "patch",
];

/// The qualified symbol for a scene node kind name, e.g. `scene/graph`.
pub fn scene_kind(name: &str) -> Symbol {
    Symbol::qualified(SCENE_NAMESPACE, name)
}

/// Is `name` a recognized baseline scene node kind (local part only)?
pub fn is_known_kind_name(name: &str) -> bool {
    SCENE_KINDS.contains(&name)
}

/// Is `symbol` a recognized baseline scene node kind (`scene/<known>`)?
pub fn is_known_kind(symbol: &Symbol) -> bool {
    symbol.namespace.as_deref() == Some(SCENE_NAMESPACE) && is_known_kind_name(&symbol.name)
}
