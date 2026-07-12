//! Intent value model: origin, builders, accessors, and fail-closed validation.
//!
//! An Intent is a SIM value: a `kind`-tagged `Expr::Map` that also carries an
//! `origin` (operator plus logical tick) and the fields its kind requires. This
//! module builds and inspects Intents over `Expr` (no parallel data model),
//! validates them structurally into a [`IntentError`], and resolves the targets
//! an Intent references against a caller-supplied predicate so an Intent naming
//! an unknown target produces a diagnostic rather than a partial mutation.

use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, Expr, NoopEvalPolicy, ShapeMatch, Symbol};

use crate::kinds::{
    AT_TICK_KEY, KIND_KEY, OPERATOR_KEY, ORIGIN_KEY, is_known_kind, required_fields,
};

/// Who issued an Intent. Recorded on every Intent for audit; both a human
/// (through the browser) and an agent (through the runner) are peers on the bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    /// A human operator gesturing through the browser shell.
    Human,
    /// An agent operator acting through the agent runner.
    Agent,
}

impl Operator {
    /// The operator symbol used inside an Intent origin.
    pub fn symbol(self) -> Symbol {
        match self {
            Operator::Human => Symbol::new("human"),
            Operator::Agent => Symbol::new("agent"),
        }
    }

    /// Parse an operator symbol, or `None` if it names neither operator.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "human" => Some(Operator::Human),
            "agent" => Some(Operator::Agent),
            _ => None,
        }
    }
}

/// The origin of an Intent: which operator issued it and at what logical tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Origin {
    /// The issuing operator.
    pub operator: Operator,
    /// A monotonically increasing logical tick.
    pub at_tick: u64,
}

impl Origin {
    /// Build an origin for a human operator at `tick`.
    pub fn human(tick: u64) -> Self {
        Self {
            operator: Operator::Human,
            at_tick: tick,
        }
    }

    /// Build an origin for an agent operator at `tick`.
    pub fn agent(tick: u64) -> Self {
        Self {
            operator: Operator::Agent,
            at_tick: tick,
        }
    }

    fn to_expr(self) -> Expr {
        sim_value::build::map(vec![
            (OPERATOR_KEY, Expr::Symbol(self.operator.symbol())),
            (AT_TICK_KEY, sim_value::build::uint(self.at_tick)),
        ])
    }
}

/// A structured Intent validation diagnostic: where the problem is and what it
/// is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntentError {
    /// Address into the Intent (for example `path` or `targets[1]`).
    pub path: Vec<String>,
    /// Human-readable description of the violation.
    pub message: String,
}

impl IntentError {
    fn at(path: &[&str], message: impl Into<String>) -> Self {
        Self {
            path: path.iter().map(|segment| (*segment).to_owned()).collect(),
            message: message.into(),
        }
    }

    /// Render the path as a dotted address, or `<root>` when empty.
    pub fn path_string(&self) -> String {
        if self.path.is_empty() {
            "<root>".to_owned()
        } else {
            self.path.join(".")
        }
    }
}

impl core::fmt::Display for IntentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.path_string(), self.message)
    }
}

/// Build an Intent value: a `kind`-tagged map carrying `origin` then `fields`.
pub fn intent(kind_name: &str, origin: Origin, fields: Vec<(&str, Expr)>) -> Expr {
    let mut pairs = Vec::with_capacity(fields.len() + 2);
    pairs.push((
        sim_value::build::sym(KIND_KEY),
        Expr::Symbol(crate::kinds::intent_kind(kind_name)),
    ));
    pairs.push((sim_value::build::sym(ORIGIN_KEY), origin.to_expr()));
    for (key, value) in fields {
        pairs.push((sim_value::build::sym(key), value));
    }
    Expr::Map(pairs)
}

fn entry<'a>(entries: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(symbol) if &*symbol.name == name && symbol.namespace.is_none())
            .then_some(value)
    })
}

/// If `expr` is a `kind`-tagged map, return the kind symbol.
pub fn intent_kind_of(expr: &Expr) -> Option<Symbol> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    match entry(entries, KIND_KEY) {
        Some(Expr::Symbol(kind)) => Some(kind.clone()),
        _ => None,
    }
}

/// Read a top-level Intent field by name.
pub fn field<'a>(expr: &'a Expr, name: &str) -> Option<&'a Expr> {
    sim_value::access::field(expr, name)
}

/// Parse the origin of an Intent, if present and well-formed.
pub fn origin(expr: &Expr) -> Option<Origin> {
    let origin = field(expr, ORIGIN_KEY)?;
    let Expr::Map(entries) = origin else {
        return None;
    };
    let operator = match entry(entries, OPERATOR_KEY) {
        Some(Expr::Symbol(symbol)) => Operator::from_name(&symbol.name)?,
        _ => return None,
    };
    let at_tick = match entry(entries, AT_TICK_KEY) {
        Some(Expr::Number(number)) => number.canonical.parse::<u64>().ok()?,
        _ => return None,
    };
    Some(Origin { operator, at_tick })
}

/// Validate that `expr` is a structurally well-formed Intent, failing closed
/// with an [`IntentError`] otherwise.
pub fn validate_intent(expr: &Expr) -> Result<(), IntentError> {
    let shape_error = check_intent_shape(expr)?;
    let Expr::Map(entries) = expr else {
        return Err(IntentError::at(&[], "an Intent must be a map"));
    };
    let kind = match entry(entries, KIND_KEY) {
        Some(Expr::Symbol(kind)) if is_known_kind(kind) => kind.clone(),
        Some(Expr::Symbol(kind)) => {
            return Err(IntentError::at(
                &[KIND_KEY],
                format!("unrecognized Intent kind '{kind}'"),
            ));
        }
        Some(_) => {
            return Err(IntentError::at(
                &[KIND_KEY],
                "Intent 'kind' must be a symbol",
            ));
        }
        None => return Err(IntentError::at(&[], "Intent is missing a 'kind' tag")),
    };
    if let Some(message) = shape_error {
        return Err(IntentError::at(&[], message));
    }
    validate_origin(entries)?;
    for required in required_fields(&kind.name) {
        let Some(value) = entry(entries, required) else {
            return Err(IntentError::at(
                &[required],
                format!("Intent '{kind}' is missing required field '{required}'"),
            ));
        };
        if *required == "path" && !matches!(value, Expr::List(_)) {
            return Err(IntentError::at(
                &["path"],
                "edit-field 'path' must be a list of segments",
            ));
        }
    }
    Ok(())
}

fn check_intent_shape(expr: &Expr) -> Result<Option<String>, IntentError> {
    let mut cx = Cx::new(Arc::new(NoopEvalPolicy), Arc::new(DefaultFactory));
    let matched = crate::shapes::intent_shape()
        .check_expr(&mut cx, expr)
        .map_err(|error| IntentError::at(&[], format!("Intent shape check failed: {error}")))?;
    Ok(
        (!matched.accepted)
            .then(|| rejection_message(&matched, "value is not a recognized Intent")),
    )
}

fn rejection_message(matched: &ShapeMatch, fallback: &str) -> String {
    matched
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| fallback.to_owned())
}

fn validate_origin(entries: &[(Expr, Expr)]) -> Result<(), IntentError> {
    let Some(origin) = entry(entries, ORIGIN_KEY) else {
        return Err(IntentError::at(
            &[ORIGIN_KEY],
            "Intent is missing an 'origin'",
        ));
    };
    let Expr::Map(origin_entries) = origin else {
        return Err(IntentError::at(
            &[ORIGIN_KEY],
            "Intent 'origin' must be a map",
        ));
    };
    match entry(origin_entries, OPERATOR_KEY) {
        Some(Expr::Symbol(symbol)) if Operator::from_name(&symbol.name).is_some() => {}
        _ => {
            return Err(IntentError::at(
                &[ORIGIN_KEY, OPERATOR_KEY],
                "origin 'operator' must be 'human' or 'agent'",
            ));
        }
    }
    match entry(origin_entries, AT_TICK_KEY) {
        Some(Expr::Number(_)) => Ok(()),
        _ => Err(IntentError::at(
            &[ORIGIN_KEY, AT_TICK_KEY],
            "origin 'at-tick' must be a number",
        )),
    }
}

/// Resolve every target an Intent references against `is_known`, returning a
/// diagnostic for the first unknown target. A failed resolution means the
/// editor must not produce an operation: nothing mutates.
pub fn resolve_targets(expr: &Expr, is_known: impl Fn(&Expr) -> bool) -> Result<(), IntentError> {
    for (label, target) in referenced_targets(expr) {
        if !is_known(&target) {
            return Err(IntentError {
                path: vec![label],
                message: "Intent references an unknown target".to_owned(),
            });
        }
    }
    Ok(())
}

/// The list of `(field-label, target-expr)` references an Intent carries, by
/// kind. Port references (`from`/`to`) contribute their inner `node`.
pub fn referenced_targets(expr: &Expr) -> Vec<(String, Expr)> {
    let Some(kind) = intent_kind_of(expr) else {
        return Vec::new();
    };
    let mut refs = Vec::new();
    let mut single = |name: &str| {
        if let Some(value) = field(expr, name) {
            refs.push((name.to_owned(), value.clone()));
        }
    };
    match &*kind.name {
        "tap" | "edit-field" | "invoke" | "scrub" | "set-param" | "performance-event"
        | "piano-roll-edit" | "player-rack-edit" | "arranger-edit" => single("target"),
        "move" => single("node"),
        "unwire" => single("edge"),
        "create" => single("class"),
        "open" => single("value"),
        "approve" | "reject" | "ask" | "split-mission" | "pause-agent" | "rerun-validation"
        | "replay-cassette" => single("mission"),
        "open-source" => single("location"),
        "select" | "delete" => {
            if let Some(Expr::List(items)) = field(expr, "targets") {
                for (index, item) in items.iter().enumerate() {
                    refs.push((format!("targets[{index}]"), item.clone()));
                }
            }
        }
        "wire" => {
            for end in ["from", "to"] {
                if let Some(Expr::Map(port)) = field(expr, end)
                    && let Some(node) = entry(port, "node")
                {
                    refs.push((format!("{end}.node"), node.clone()));
                }
            }
        }
        _ => {}
    }
    refs
}
