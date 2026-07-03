//! Shapes for Intent kinds.
//!
//! Each baseline Intent kind gets a Shape that matches a `kind`-tagged
//! `Expr::Map` for that kind; an umbrella `intent/Intent` Shape matches any
//! recognized Intent and is used as the `codec:intent` expression shape. Editor
//! dispatch (WEBUI_4 P3) selects editors by Shape match over these, reusing the
//! kernel matcher.

use std::sync::Arc;

use sim_kernel::{Cx, Expr, MatchScore, Result, Shape, ShapeDoc, ShapeMatch, Symbol, Value};

use crate::kinds::{INTENT_KINDS, INTENT_NAMESPACE, intent_kind, is_known_kind};
use crate::model::intent_kind_of;

/// A Shape that accepts exactly one Intent kind.
pub struct IntentKindShape {
    kind: Symbol,
    symbol: Symbol,
}

impl IntentKindShape {
    /// Build the Shape for the Intent kind named `name` (e.g. `wire`).
    pub fn new(name: &str) -> Self {
        Self {
            kind: intent_kind(name),
            symbol: Symbol::qualified(INTENT_NAMESPACE, pascal_case(name)),
        }
    }
}

impl Shape for IntentKindShape {
    fn symbol(&self) -> Option<Symbol> {
        Some(self.symbol.clone())
    }

    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        let expr = value.object().as_expr(cx)?;
        self.check_expr(cx, &expr)
    }

    fn check_expr(&self, _cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        match intent_kind_of(expr) {
            Some(kind) if kind == self.kind => Ok(ShapeMatch::accept(MatchScore::exact(20))),
            Some(kind) => Ok(ShapeMatch::reject(format!(
                "Intent kind '{kind}' does not match '{}'",
                self.kind
            ))),
            None => Ok(ShapeMatch::reject("value is not an Intent")),
        }
    }

    fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
        Ok(ShapeDoc::new(self.symbol.name.to_string())
            .with_detail(format!("matches Intents tagged '{}'", self.kind)))
    }
}

/// The umbrella Shape that accepts any recognized Intent.
pub struct IntentShape;

impl Shape for IntentShape {
    fn symbol(&self) -> Option<Symbol> {
        Some(intent_shape_symbol())
    }

    fn check_value(&self, cx: &mut Cx, value: Value) -> Result<ShapeMatch> {
        let expr = value.object().as_expr(cx)?;
        self.check_expr(cx, &expr)
    }

    fn check_expr(&self, _cx: &mut Cx, expr: &Expr) -> Result<ShapeMatch> {
        match intent_kind_of(expr) {
            Some(kind) if is_known_kind(&kind) => Ok(ShapeMatch::accept(MatchScore::exact(5))),
            Some(kind) => Ok(ShapeMatch::reject(format!(
                "unrecognized Intent kind '{kind}'"
            ))),
            None => Ok(ShapeMatch::reject("value is not an Intent")),
        }
    }

    fn describe(&self, _cx: &mut Cx) -> Result<ShapeDoc> {
        Ok(ShapeDoc::new("Intent").with_detail("any recognized Intent (a kind-tagged map)"))
    }
}

/// The symbol for the umbrella `intent/Intent` Shape.
pub fn intent_shape_symbol() -> Symbol {
    Symbol::qualified(INTENT_NAMESPACE, "Intent")
}

/// Build `(symbol, shape)` registrations for the umbrella Shape plus every
/// baseline Intent kind Shape.
pub fn intent_shape_specs() -> Vec<(Symbol, Arc<dyn Shape>)> {
    let mut specs: Vec<(Symbol, Arc<dyn Shape>)> =
        vec![(intent_shape_symbol(), Arc::new(IntentShape))];
    for name in INTENT_KINDS {
        let shape = IntentKindShape::new(name);
        specs.push((shape.symbol.clone(), Arc::new(shape)));
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
