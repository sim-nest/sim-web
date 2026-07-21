//! Scene construction helpers.
//!
//! Re-exports the `sim-value` builders and adds a few common scene node shapes
//! plus a reserved-key guard. The guard turns the `kind` footgun into an
//! immediate, clear failure: `kind` is the scene-node tag, so a plain data map
//! must not carry a `kind` key (use a different field name). [`data_map`]
//! debug-asserts that.

use sim_kernel::Expr;

pub use sim_value::build::{float, int, list, map, sym, text, vector};

use crate::model::node;

/// Keys reserved for scene-node structure; plain data maps must not use them.
pub const RESERVED_DATA_KEYS: &[&str] = &["kind"];

/// A `scene/stack` node with a direction and children.
pub fn stack(dir: &str, children: Vec<Expr>) -> Expr {
    node(
        "stack",
        vec![("dir", sym(dir)), ("children", list(children))],
    )
}

/// A `scene/box` node with a role and children.
pub fn box_(role: &str, children: Vec<Expr>) -> Expr {
    node(
        "box",
        vec![("role", sym(role)), ("children", list(children))],
    )
}

/// A `scene/badge` node. Status carries a text token, never color alone.
pub fn badge(status: &str, label: &str) -> Expr {
    node(
        "badge",
        vec![("status", sym(status)), ("label", text(label))],
    )
}

/// A `scene/badge-cluster` node containing visible status badges.
pub fn badge_cluster(badges: Vec<Expr>) -> Expr {
    node("badge-cluster", vec![("badges", list(badges))])
}

/// A `scene/text` node.
pub fn text_node(content: impl Into<String>) -> Expr {
    node("text", vec![("text", text(content.into()))])
}

/// Build a plain data map, asserting (in debug) that it carries no reserved
/// scene-node key.
pub fn data_map(entries: Vec<(&str, Expr)>) -> Expr {
    debug_assert!(
        entries
            .iter()
            .all(|(key, _)| !RESERVED_DATA_KEYS.contains(key)),
        "data_map: a plain data map must not use a reserved scene-node key (e.g. 'kind'); \
         rename the field"
    );
    map(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_shape_helpers_validate() {
        let scene = stack(
            "column",
            vec![box_(
                "summary",
                vec![text_node("hi"), badge_cluster(vec![badge("ok", "done")])],
            )],
        );
        crate::model::validate_scene(&scene).expect("helper scenes validate");
    }

    #[test]
    fn data_map_allows_non_reserved_keys() {
        let value = data_map(vec![("style", sym("line")), ("at", int(3))]);
        assert!(matches!(value, Expr::Map(_)));
    }

    #[test]
    #[should_panic(expected = "reserved scene-node key")]
    fn data_map_rejects_a_reserved_key_in_debug() {
        let _ = data_map(vec![("kind", sym("line"))]);
    }
}
