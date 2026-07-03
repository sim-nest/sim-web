//! Symbolic-expression tree lens.
//!
//! A symbolic expression is a SIM `Expr` (calls, symbols, numbers). The lens
//! renders it as a `scene/tree`: each operator is a branch labelled by its head,
//! each leaf an atom. It never evaluates the expression; it shows its structure.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::node;

/// The symbolic lens id.
pub const SYMBOLIC_LENS: &str = "view:math-symbolic";

/// Render a symbolic expression as a `scene/tree`.
pub fn symbolic_tree(expr: &Expr) -> Expr {
    match expr {
        Expr::Call { operator, args } => {
            let mut nodes = vec![leaf(&format!("op: {}", render_head(operator)))];
            nodes.extend(args.iter().map(symbolic_tree));
            branch(&render_head(operator), nodes)
        }
        Expr::Infix {
            operator,
            left,
            right,
        } => branch(
            &operator.to_string(),
            vec![symbolic_tree(left), symbolic_tree(right)],
        ),
        Expr::Prefix { operator, arg } | Expr::Postfix { operator, arg } => {
            branch(&operator.to_string(), vec![symbolic_tree(arg)])
        }
        Expr::List(items) | Expr::Vector(items) => {
            branch("seq", items.iter().map(symbolic_tree).collect())
        }
        Expr::Symbol(symbol) | Expr::Local(symbol) => leaf(&symbol.as_qualified_str()),
        Expr::Number(number) => leaf(&number.canonical),
        Expr::String(text) => leaf(&format!("{text:?}")),
        Expr::Bool(flag) => leaf(&flag.to_string()),
        Expr::Nil => leaf("nil"),
        other => leaf(&format!("{other:?}")),
    }
}

fn render_head(operator: &Expr) -> String {
    match operator {
        Expr::Symbol(symbol) => symbol.as_qualified_str(),
        other => format!("{other:?}"),
    }
}

fn branch(label: &str, nodes: Vec<Expr>) -> Expr {
    node(
        "tree",
        vec![
            ("label", Expr::String(label.to_owned())),
            ("nodes", Expr::List(nodes)),
        ],
    )
}

fn leaf(text: &str) -> Expr {
    node("text", vec![("text", Expr::String(text.to_owned()))])
}

/// A small symbolic builder for tests and demos: `(operator arg ...)`.
pub fn call(operator: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        operator: Box::new(Expr::Symbol(Symbol::new(operator))),
        args,
    }
}
