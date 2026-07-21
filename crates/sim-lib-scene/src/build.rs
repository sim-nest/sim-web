//! Scene construction helpers.
//!
//! Re-exports the `sim-value` builders and adds a few common scene node shapes
//! plus a reserved-key guard. The guard turns the `kind` footgun into an
//! immediate, clear failure: `kind` is the scene-node tag, so a plain data map
//! must not carry a `kind` key (use a different field name). [`data_map`]
//! debug-asserts that.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_value::access;

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

/// The stable anchor spaces used by pose-free spatial scene nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorSpace {
    /// Head-relative content, such as a near cursor plane.
    Head,
    /// World-relative content, such as a pinned panel or plane.
    World,
    /// Screen-relative content, such as a fixed overlay.
    Screen,
    /// Body-relative content, such as a chest-anchored toolbelt.
    Body,
    /// Device-relative content, such as an accessory-aligned panel.
    Device,
}

impl AnchorSpace {
    /// Returns the anchor-space token used in Scene data.
    pub fn as_name(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::World => "world",
            Self::Screen => "screen",
            Self::Body => "body",
            Self::Device => "device",
        }
    }

    /// Encodes the anchor space as a bare symbol.
    pub fn to_expr(self) -> Expr {
        sym(self.as_name())
    }

    /// Reads an anchor-space token from a Scene expression.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let Expr::Symbol(symbol) = expr else {
            return Err(Error::Eval(
                "scene/anchor space must be a symbol".to_owned(),
            ));
        };
        Self::from_symbol(symbol)
    }

    fn from_symbol(symbol: &Symbol) -> Result<Self> {
        if symbol.namespace.is_some() {
            return Err(Error::Eval(format!(
                "scene/anchor space must be unqualified, got {symbol}"
            )));
        }
        match symbol.name.as_ref() {
            "head" => Ok(Self::Head),
            "world" => Ok(Self::World),
            "screen" => Ok(Self::Screen),
            "body" => Ok(Self::Body),
            "device" => Ok(Self::Device),
            other => Err(Error::Eval(format!("unknown scene/anchor space {other}"))),
        }
    }
}

/// A named pose-free anchor for spatial scene nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    /// The coordinate space the target belongs to.
    pub space: AnchorSpace,
    /// Stable target id in that space.
    pub target: String,
}

impl Anchor {
    /// Builds a named anchor in the given space.
    pub fn new(space: AnchorSpace, target: impl Into<String>) -> Self {
        Self {
            space,
            target: target.into(),
        }
    }

    /// Encodes the anchor as a `scene/anchor` node.
    pub fn to_expr(&self) -> Expr {
        node(
            "anchor",
            vec![
                ("space", self.space.to_expr()),
                ("target", text(self.target.clone())),
            ],
        )
    }

    /// Reads a `scene/anchor` node.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        expect_kind(expr, "anchor")?;
        Ok(Self {
            space: AnchorSpace::from_expr(access::required(expr, "space", "scene/anchor")?)?,
            target: access::required_str(expr, "target", "scene/anchor")?.to_owned(),
        })
    }
}

/// A static transform attached to spatial content before device pose is known.
#[derive(Clone, Debug, PartialEq)]
pub struct Transform3 {
    /// Translation in meters.
    pub translate_m: [f64; 3],
    /// Rotation quaternion in `[x, y, z, w]` order.
    pub rotate_xyzw: [f64; 4],
    /// Scale on each axis.
    pub scale: [f64; 3],
}

impl Transform3 {
    /// Builds a transform from explicit translation, rotation, and scale.
    pub fn new(translate_m: [f64; 3], rotate_xyzw: [f64; 4], scale: [f64; 3]) -> Self {
        Self {
            translate_m,
            rotate_xyzw,
            scale,
        }
    }

    /// Returns the identity transform.
    pub fn identity() -> Self {
        Self::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0])
    }

    /// Encodes the transform as pose-free Scene data.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("translate-m", f64_vector(self.translate_m)),
            ("rotate-xyzw", f64_vector(self.rotate_xyzw)),
            ("scale", f64_vector(self.scale)),
        ])
    }

    /// Reads a transform from pose-free Scene data.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let transform = Self {
            translate_m: read_f64_vector(expr, "translate-m", "scene/Transform3")?,
            rotate_xyzw: read_f64_vector(expr, "rotate-xyzw", "scene/Transform3")?,
            scale: read_f64_vector(expr, "scale", "scene/Transform3")?,
        };
        if transform.rotate_xyzw.iter().all(|value| *value == 0.0) {
            return Err(Error::Eval(
                "scene/Transform3 rotation must not be the zero quaternion".to_owned(),
            ));
        }
        if transform.scale.contains(&0.0) {
            return Err(Error::Eval(
                "scene/Transform3 scale entries must be nonzero".to_owned(),
            ));
        }
        Ok(transform)
    }
}

/// Builds a `scene/spatial` root with pose-free spatial children.
pub fn spatial(children: Vec<Expr>) -> Expr {
    node("spatial", vec![("children", list(children))])
}

/// Builds a `scene/anchor` node.
pub fn anchor(space: AnchorSpace, target: impl Into<String>) -> Expr {
    Anchor::new(space, target).to_expr()
}

/// Builds a pose-free `scene/panel` node.
pub fn panel(id: impl Into<String>, body: Expr, anchor: Anchor, transform: Transform3) -> Expr {
    node(
        "panel",
        vec![
            ("id", text(id.into())),
            ("body", body),
            ("anchor", anchor.to_expr()),
            ("transform", transform.to_expr()),
        ],
    )
}

/// Builds a pose-free `scene/gaze-cursor` node.
pub fn gaze_cursor(anchor: Anchor, transform: Transform3) -> Expr {
    node(
        "gaze-cursor",
        vec![
            ("anchor", anchor.to_expr()),
            ("transform", transform.to_expr()),
        ],
    )
}

/// Builds a pose-free `scene/hand-ray` node.
pub fn hand_ray(hand: &str, anchor: Anchor, transform: Transform3) -> Expr {
    node(
        "hand-ray",
        vec![
            ("hand", sym(hand)),
            ("anchor", anchor.to_expr()),
            ("transform", transform.to_expr()),
        ],
    )
}

/// Builds a pose-free `scene/world-plane` node.
pub fn world_plane(
    id: impl Into<String>,
    anchor: Anchor,
    transform: Transform3,
    size_m: [f64; 2],
) -> Expr {
    node(
        "world-plane",
        vec![
            ("id", text(id.into())),
            ("anchor", anchor.to_expr()),
            ("transform", transform.to_expr()),
            ("size-m", f64_vector(size_m)),
        ],
    )
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

fn expect_kind(expr: &Expr, expected: &str) -> Result<()> {
    match crate::model::node_kind(expr) {
        Some(kind)
            if kind.namespace.as_deref() == Some(crate::kinds::SCENE_NAMESPACE)
                && kind.name.as_ref() == expected =>
        {
            Ok(())
        }
        _ => Err(Error::Eval(format!("expected scene/{expected} node"))),
    }
}

fn f64_vector<const N: usize>(values: [f64; N]) -> Expr {
    vector(values.into_iter().map(float).collect())
}

fn read_f64_vector<const N: usize>(expr: &Expr, name: &str, context: &str) -> Result<[f64; N]> {
    let Expr::Vector(items) = access::required(expr, name, context)? else {
        return Err(Error::Eval(format!(
            "{context} field {name} is not a vector"
        )));
    };
    if items.len() != N {
        return Err(Error::Eval(format!(
            "{context} field {name} must contain {N} numbers"
        )));
    }
    let mut out = [0.0; N];
    for (index, item) in items.iter().enumerate() {
        let value = access::as_f64(item)
            .ok_or_else(|| Error::Eval(format!("{context} field {name}[{index}] is not f64")))?;
        if !value.is_finite() {
            return Err(Error::Eval(format!(
                "{context} field {name}[{index}] is not finite"
            )));
        }
        out[index] = value;
    }
    Ok(out)
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
