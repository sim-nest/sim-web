//! Shapes for Intent kinds.
//!
//! Each baseline Intent kind gets a Shape that matches a `kind`-tagged
//! `Expr::Map` for that kind; an umbrella `intent/Intent` Shape matches any
//! recognized Intent and is used as the `codec:intent` expression shape. Editor
//! dispatch selects editors by Shape match over these, reusing the kernel
//! matcher.

use std::sync::Arc;

use sim_kernel::{Cx, Expr, MatchScore, Result, Shape, ShapeDoc, ShapeMatch, Symbol, Value};
use sim_shape::{ExactExprShape, OrShape, TableExtraPolicy, TableFieldSpec, TableShape};

use crate::kinds::{INTENT_KINDS, INTENT_NAMESPACE, KIND_KEY, intent_kind};

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

/// The symbol for the umbrella `intent/Intent` Shape.
pub fn intent_shape_symbol() -> Symbol {
    Symbol::qualified(INTENT_NAMESPACE, "Intent")
}

pub(crate) fn intent_shape() -> Arc<dyn Shape> {
    let choices = INTENT_KINDS
        .iter()
        .map(|name| kind_field_shape(intent_kind(name)))
        .collect();
    ranked_shape(
        intent_shape_symbol(),
        "Intent",
        "any recognized Intent (a kind-tagged map)",
        5,
        Arc::new(OrShape::new(choices)),
    )
}

fn intent_kind_shape(name: &str) -> (Symbol, Arc<dyn Shape>) {
    let symbol = Symbol::qualified(INTENT_NAMESPACE, pascal_case(name));
    let kind = intent_kind(name);
    let shape = ranked_shape(
        symbol.clone(),
        symbol.name.to_string(),
        format!("matches Intents tagged '{kind}'"),
        20,
        kind_field_shape(kind),
    );
    (symbol, shape)
}

/// Build `(symbol, shape)` registrations for the umbrella Shape plus every
/// baseline Intent kind Shape.
pub fn intent_shape_specs() -> Vec<(Symbol, Arc<dyn Shape>)> {
    let mut specs: Vec<(Symbol, Arc<dyn Shape>)> = vec![(intent_shape_symbol(), intent_shape())];
    for name in INTENT_KINDS {
        specs.push(intent_kind_shape(name));
    }
    specs
}

/// Convert a kebab-case kind name to PascalCase for the Shape symbol.
fn pascal_case(name: &str) -> String {
    name.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}
