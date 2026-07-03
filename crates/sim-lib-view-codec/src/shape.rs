//! The Shape lens: matcher visualization, bindings, and counterexamples.
//!
//! Shapes are one of SIM's strongest differentiators, so this lens exposes them
//! directly: it shows what a Shape is (its documentation), whether a value
//! matches (with the captured bindings), and a failing-match counterexample with
//! the diagnostics that explain the rejection.

use sim_kernel::{Cx, Expr, Result, Symbol, Value};
use sim_lib_scene::{node, sym};

/// The Shape lens id.
pub const SHAPE_LENS: &str = "view:codec-shape";

/// Render a Shape inspected against a matching `value` and a `counterexample`
/// that should fail, into one Scene.
pub fn shape_view(
    cx: &mut Cx,
    shape_value: &Value,
    value: &Expr,
    counterexample: &Expr,
) -> Result<Expr> {
    let shape = shape_value
        .object()
        .as_shape()
        .ok_or_else(|| sim_kernel::Error::HostError("value is not a Shape".to_owned()))?;

    let doc = shape.describe(cx)?;
    let matched = shape.check_expr(cx, value)?;
    let counter = shape.check_expr(cx, counterexample)?;

    let mut sections = vec![
        // Matcher description (matcher-tree visualization).
        matcher_tree(&doc),
        // The match result for the example value.
        match_section(
            "match",
            value,
            matched.accepted,
            &bindings(&matched.captures),
            &matched.diagnostics,
        ),
    ];
    // The counterexample: it must be rejected; its diagnostics explain why.
    sections.push(match_section(
        "counterexample",
        counterexample,
        counter.accepted,
        &[],
        &counter.diagnostics,
    ));

    Ok(node(
        "box",
        vec![
            ("role", sym("shape")),
            (
                "shape",
                Expr::Symbol(
                    shape
                        .symbol()
                        .unwrap_or_else(|| Symbol::new(doc.name.clone())),
                ),
            ),
            ("children", Expr::List(sections)),
        ],
    ))
}

fn matcher_tree(doc: &sim_kernel::ShapeDoc) -> Expr {
    let details = doc
        .details
        .iter()
        .map(|detail| node("text", vec![("text", Expr::String(detail.clone()))]))
        .collect();
    node(
        "tree",
        vec![
            ("label", Expr::String(format!("shape: {}", doc.name))),
            ("nodes", Expr::List(details)),
        ],
    )
}

fn match_section(
    role: &str,
    value: &Expr,
    accepted: bool,
    bindings: &[String],
    diagnostics: &[sim_kernel::Diagnostic],
) -> Expr {
    let mut children = vec![
        node(
            "text",
            vec![("text", Expr::String(format!("value: {}", render(value))))],
        ),
        node(
            "badge",
            vec![
                ("status", sym(if accepted { "ok" } else { "error" })),
                (
                    "label",
                    Expr::String(if accepted { "matches" } else { "rejected" }.to_owned()),
                ),
            ],
        ),
    ];
    if !bindings.is_empty() {
        children.push(node(
            "text",
            vec![(
                "text",
                Expr::String(format!("bindings: {}", bindings.join(", "))),
            )],
        ));
    }
    for diagnostic in diagnostics {
        children.push(node(
            "text",
            vec![
                ("role", sym("diagnostic")),
                ("text", Expr::String(diagnostic.message.clone())),
            ],
        ));
    }
    node(
        "box",
        vec![("role", sym(role)), ("children", Expr::List(children))],
    )
}

fn bindings(captures: &sim_kernel::ShapeBindings) -> Vec<String> {
    captures
        .exprs()
        .iter()
        .map(|(name, _)| name.to_string())
        .chain(captures.values().iter().map(|(name, _)| name.to_string()))
        .collect()
}

fn render(value: &Expr) -> String {
    match value {
        Expr::Symbol(symbol) => symbol.as_qualified_str(),
        Expr::Number(number) => number.canonical.clone(),
        Expr::String(text) => format!("{text:?}"),
        Expr::Bool(flag) => flag.to_string(),
        Expr::Nil => "nil".to_owned(),
        other => format!("<{other:?}>"),
    }
}
