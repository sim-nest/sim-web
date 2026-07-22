//! Intent kinds and their required fields.
//!
//! An Intent is an `Expr::Map` tagged with a `kind` symbol in the `intent`
//! namespace (for example `intent/wire`). Kinds are open metadata, not a closed
//! kernel enum. This module lists the baseline Intent kinds and the fields each
//! kind must carry, which the validator uses to fail closed on a malformed
//! Intent.

use sim_kernel::Symbol;

/// The namespace every Intent `kind` symbol lives in.
pub const INTENT_NAMESPACE: &str = "intent";

/// The map key that tags an Intent with its kind.
pub const KIND_KEY: &str = "kind";

/// The map key carrying the Intent origin (operator + logical tick).
pub const ORIGIN_KEY: &str = "origin";

/// The origin sub-key naming the operator (`human` or `agent`).
pub const OPERATOR_KEY: &str = "operator";

/// The origin sub-key carrying the logical tick.
pub const AT_TICK_KEY: &str = "at-tick";

/// The baseline Intent kind names (the local part of the `intent/*` symbol).
pub const INTENT_KINDS: &[&str] = &[
    "tap",
    "select",
    "edit",
    "edit-field",
    "move",
    "wire",
    "unwire",
    "create",
    "delete",
    "invoke",
    "dismiss",
    "set-lens",
    "set-mode",
    "open",
    "commit",
    "cancel",
    "scrub",
    "set-param",
    "performance-event",
    "piano-roll-edit",
    "player-rack-edit",
    "arranger-edit",
    "approve",
    "reject",
    "ask",
    "split-mission",
    "pause-agent",
    "rerun-validation",
    "replay-cassette",
    "open-source",
];

/// The qualified symbol for an Intent kind name, e.g. `intent/wire`.
pub fn intent_kind(name: &str) -> Symbol {
    Symbol::qualified(INTENT_NAMESPACE, name)
}

/// Is `name` a recognized baseline Intent kind (local part only)?
pub fn is_known_kind_name(name: &str) -> bool {
    INTENT_KINDS.contains(&name)
}

/// Is `symbol` a recognized baseline Intent kind (`intent/<known>`)?
pub fn is_known_kind(symbol: &Symbol) -> bool {
    symbol.namespace.as_deref() == Some(INTENT_NAMESPACE) && is_known_kind_name(&symbol.name)
}

/// The fields a given Intent kind must carry (besides `kind` and `origin`).
pub fn required_fields(kind_name: &str) -> &'static [&'static str] {
    match kind_name {
        "tap" => &["target", "control"],
        "select" => &["targets"],
        "edit" => &["target", "path"],
        "edit-field" => &["target", "path", "value"],
        "move" => &["node", "at"],
        "wire" => &["from", "to"],
        "unwire" => &["edge"],
        "create" => &["class", "at", "args"],
        "delete" => &["targets"],
        "invoke" => &["target", "op", "args"],
        "dismiss" => &[],
        "set-lens" => &["pane", "lens"],
        "set-mode" => &["mode"],
        "open" => &["value", "pane"],
        "commit" => &["pane"],
        "cancel" => &["pane"],
        "scrub" => &["target", "at"],
        "set-param" => &["target", "param", "value"],
        "performance-event" => &["target", "source", "input", "event"],
        "piano-roll-edit" => &["target", "action"],
        "player-rack-edit" => &["target", "action"],
        "arranger-edit" => &["target", "action"],
        "approve" => &["mission"],
        "reject" => &["mission"],
        "ask" => &["mission", "question"],
        "split-mission" => &["mission", "goals"],
        "pause-agent" => &["mission"],
        "rerun-validation" => &["mission"],
        "replay-cassette" => &["mission", "at"],
        "open-source" => &["location"],
        _ => &[],
    }
}
