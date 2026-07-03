//! Shapes for scene node kinds.
//!
//! Each baseline scene node kind gets a Shape that matches an `Expr::Map` whose
//! `kind` tag equals that kind. View selection (WEBUI_4 P3) is overload
//! selection over these Shapes, so the same matcher the kernel already uses for
//! dispatch chooses lenses; there is no separate selection ladder. An umbrella
//! `scene/Scene` Shape matches any recognized scene node and is used as the
//! `codec:scene` expression shape.

use sim_kernel::{Cx, Expr, MatchScore, Result, Shape, ShapeDoc, ShapeMatch, Symbol, Value};

use crate::kinds::{SCENE_KINDS, SCENE_NAMESPACE, is_known_kind, scene_kind};
use crate::model::node_kind;

/// A Shape that accepts exactly one scene node kind.
pub struct SceneNodeShape {
    kind: Symbol,
    symbol: Symbol,
}

impl SceneNodeShape {
    /// Build the Shape for the scene node kind named `name` (e.g. `graph`).
    pub fn new(name: &str) -> Self {
        Self {
            kind: scene_kind(name),
            symbol: Symbol::qualified(SCENE_NAMESPACE, capitalize(name)),
        }
    }
}

impl Shape for SceneNodeShape {
    fn symbol(&self) -> Option<Symbol> {
        Some(self.symbol.clone())
    }

    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        let expr = value.object().as_expr(cx)?;
        self.check_expr(cx, &expr)
    }

    fn check_expr(&self, _cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        match node_kind(expr) {
            Some(kind) if kind == self.kind => Ok(ShapeMatch::accept(MatchScore::exact(20))),
            Some(kind) => Ok(ShapeMatch::reject(format!(
                "scene node kind '{kind}' does not match '{}'",
                self.kind
            ))),
            None => Ok(ShapeMatch::reject("value is not a scene node")),
        }
    }

    fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
        Ok(ShapeDoc::new(self.symbol.name.to_string())
            .with_detail(format!("matches scene nodes tagged '{}'", self.kind)))
    }
}

/// The umbrella Shape that accepts any recognized scene node.
pub struct SceneShape;

impl Shape for SceneShape {
    fn symbol(&self) -> Option<Symbol> {
        Some(Symbol::qualified(SCENE_NAMESPACE, "Scene"))
    }

    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        let expr = value.object().as_expr(cx)?;
        self.check_expr(cx, &expr)
    }

    fn check_expr(&self, _cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        match node_kind(expr) {
            Some(kind) if is_known_kind(&kind) => Ok(ShapeMatch::accept(MatchScore::exact(5))),
            Some(kind) => Ok(ShapeMatch::reject(format!(
                "unrecognized scene kind '{kind}'"
            ))),
            None => Ok(ShapeMatch::reject("value is not a scene node")),
        }
    }

    fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
        Ok(ShapeDoc::new("Scene").with_detail("any recognized scene node (a kind-tagged map)"))
    }
}

/// The symbol for the umbrella `scene/Scene` Shape.
pub fn scene_shape_symbol() -> Symbol {
    Symbol::qualified(SCENE_NAMESPACE, "Scene")
}

/// Build `(symbol, shape)` registrations for the umbrella Shape plus every
/// baseline scene node kind Shape.
pub fn scene_shape_specs() -> Vec<(Symbol, std::sync::Arc<dyn Shape>)> {
    let mut specs: Vec<(Symbol, std::sync::Arc<dyn Shape>)> =
        vec![(scene_shape_symbol(), std::sync::Arc::new(SceneShape))];
    for name in SCENE_KINDS {
        let shape = SceneNodeShape::new(name);
        specs.push((shape.symbol.clone(), std::sync::Arc::new(shape)));
    }
    specs
}

fn capitalize(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}
