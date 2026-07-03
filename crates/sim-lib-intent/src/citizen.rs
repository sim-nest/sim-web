use sim_citizen_derive::Citizen;
use sim_kernel::{Expr, Result, Symbol};

use crate::{Origin, intent};

/// A validated Intent wrapped as a runtime Citizen object.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "intent/Intent", version = 1)]
pub struct IntentDescriptor {
    #[citizen(with = "intent_expr")]
    intent: Expr,
}

impl IntentDescriptor {
    /// Builds a descriptor from an Intent expression, validating it.
    ///
    /// # Errors
    ///
    /// Returns an error when `intent` is not a well-formed Intent expression.
    pub fn from_expr(intent: Expr) -> Result<Self> {
        intent_expr::decode(&intent)?;
        Ok(Self { intent })
    }

    /// Returns the underlying Intent expression.
    pub fn as_expr(&self) -> &Expr {
        &self.intent
    }
}

impl Default for IntentDescriptor {
    fn default() -> Self {
        Self::from_expr(intent(
            "set-lens",
            Origin::human(1),
            vec![
                ("pane", Expr::Symbol(Symbol::new("pane-1"))),
                ("lens", Expr::Symbol(Symbol::new("view:default"))),
            ],
        ))
        .expect("default intent descriptor should be valid")
    }
}

/// Returns the class symbol for the Intent descriptor Citizen.
pub fn intent_descriptor_class_symbol() -> Symbol {
    Symbol::qualified("intent", "Intent")
}

pub(crate) mod intent_expr {
    use sim_kernel::{Error, Expr, Result};

    use crate::validate_intent;

    pub fn encode(expr: &Expr) -> Expr {
        expr.clone()
    }

    pub fn decode(expr: &Expr) -> Result<Expr> {
        validate_intent(expr).map_err(|error| Error::Eval(format!("malformed intent: {error}")))?;
        Ok(expr.clone())
    }
}
