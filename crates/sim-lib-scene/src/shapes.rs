//! Shapes for scene node kinds.
//!
//! Each baseline scene node kind gets a Shape that matches an `Expr::Map` whose
//! `kind` tag equals that kind. View selection is overload selection over these
//! Shapes, so the same matcher the kernel already uses for dispatch chooses
//! lenses; there is no separate selection ladder. An umbrella `scene/Scene`
//! Shape matches any recognized scene node and is used as the `codec:scene`
//! expression shape.

use std::sync::Arc;

use sim_kernel::{Cx, Expr, MatchScore, Result, Shape, ShapeDoc, ShapeMatch, Symbol, Value};
use sim_shape::{ExactExprShape, OrShape, TableExtraPolicy, TableFieldSpec, TableShape};

use crate::kinds::{KIND_KEY, SCENE_KINDS, SCENE_NAMESPACE, scene_kind};

struct RankedShape {
    symbol: Symbol,
    name: String,
    detail: String,
    score: MatchScore,
    inner: Arc<dyn Shape>,
}

impl RankedShape {
    fn ranked(&self, mut matched: ShapeMatch) -> ShapeMatch {
        if matched.accepted {
            matched.score = self.score;
        }
        matched
    }
}

impl Shape for RankedShape {
    fn symbol(&self) -> Option<Symbol> {
        Some(self.symbol.clone())
    }

    fn is_effectful(&self) -> bool {
        self.inner.is_effectful()
    }

    fn is_total(&self) -> bool {
        self.inner.is_total()
    }

    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        self.inner
            .check_value(cx, value)
            .map(|matched| self.ranked(matched))
    }

    fn check_expr(&self, cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        self.inner
            .check_expr(cx, expr)
            .map(|matched| self.ranked(matched))
    }

    fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
        Ok(ShapeDoc::new(self.name.clone()).with_detail(self.detail.clone()))
    }
}

fn kind_field_shape(kind: Symbol) -> Arc<dyn Shape> {
    Arc::new(TableShape::new(
        vec![TableFieldSpec {
            key: Symbol::new(KIND_KEY),
            shape: Arc::new(ExactExprShape::new(Expr::Symbol(kind))),
            required: true,
        }],
        TableExtraPolicy::Allow,
    ))
}

fn ranked_shape(
    symbol: Symbol,
    name: impl Into<String>,
    detail: impl Into<String>,
    score: i32,
    inner: Arc<dyn Shape>,
) -> Arc<dyn Shape> {
    Arc::new(RankedShape {
        symbol,
        name: name.into(),
        detail: detail.into(),
        score: MatchScore::exact(score),
        inner,
    })
}

/// The symbol for the umbrella `scene/Scene` Shape.
pub fn scene_shape_symbol() -> Symbol {
    Symbol::qualified(SCENE_NAMESPACE, "Scene")
}

pub(crate) fn scene_shape() -> Arc<dyn Shape> {
    let choices = SCENE_KINDS
        .iter()
        .map(|name| kind_field_shape(scene_kind(name)))
        .collect();
    ranked_shape(
        scene_shape_symbol(),
        "Scene",
        "any recognized scene node (a kind-tagged map)",
        5,
        Arc::new(OrShape::new(choices)),
    )
}

fn scene_node_shape(name: &str) -> (Symbol, Arc<dyn Shape>) {
    let symbol = Symbol::qualified(SCENE_NAMESPACE, capitalize(name));
    let kind = scene_kind(name);
    let shape = ranked_shape(
        symbol.clone(),
        symbol.name.to_string(),
        format!("matches scene nodes tagged '{kind}'"),
        20,
        kind_field_shape(kind),
    );
    (symbol, shape)
}

/// Build `(symbol, shape)` registrations for the umbrella Shape plus every
/// baseline scene node kind Shape.
pub fn scene_shape_specs() -> Vec<(Symbol, Arc<dyn Shape>)> {
    let mut specs: Vec<(Symbol, Arc<dyn Shape>)> = vec![(scene_shape_symbol(), scene_shape())];
    for name in SCENE_KINDS {
        specs.push(scene_node_shape(name));
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
