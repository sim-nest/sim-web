//! The universal default view: a complete Scene for any value with no
//! specialized lens.
//!
//! Every value must open even when nothing specialized claims it. This view
//! emits a four-region Scene -- a summary card, a structure tree, the canonical
//! text, and an operations inspector -- built only from baseline scene node
//! kinds, so it is shipped and polished rather than a stub.

use sim_kernel::{CodecId, Cx, Expr, Result};
use sim_lib_scene::{SceneBudget, SceneBudgetExhausted, SceneBudgetState, node, sym};

use crate::contract::View;

/// The universal default view object.
pub struct UniversalView;

impl View for UniversalView {
    fn encode(&self, _cx: &mut Cx, value: &Expr) -> Result<Expr> {
        Ok(node(
            "stack",
            vec![
                ("id", sym("universal")),
                ("dir", sym("column")),
                (
                    "children",
                    Expr::List(vec![
                        summary_card(value),
                        structure_tree(value),
                        canonical_text(value),
                        operations_inspector(value),
                    ]),
                ),
            ],
        ))
    }
}

fn text_line(text: String) -> Expr {
    node("text", vec![("text", Expr::String(text))])
}

fn badge(status: &str, label: &str) -> Expr {
    // Status carries a text token; it never relies on color alone.
    node(
        "badge",
        vec![
            ("status", sym(status)),
            ("label", Expr::String(label.to_owned())),
        ],
    )
}

/// Region 1: class/identity/kind/round-trip summary.
fn summary_card(value: &Expr) -> Expr {
    let roundtrip = roundtrip_badge(value);
    node(
        "box",
        vec![
            ("role", sym("summary")),
            (
                "children",
                Expr::List(vec![
                    text_line(format!("kind: {}", expr_kind(value))),
                    text_line(format!("label: {}", short_label(value))),
                    roundtrip,
                ]),
            ),
        ],
    )
}

/// Region 2: an expandable structure tree.
fn structure_tree(value: &Expr) -> Expr {
    let mut budget = SceneBudgetState::new(SceneBudget::interactive());
    node(
        "box",
        vec![
            ("role", sym("structure")),
            (
                "children",
                Expr::List(vec![tree_of("value", value, &mut budget, 0, Vec::new())]),
            ),
        ],
    )
}

fn tree_of(
    label: &str,
    value: &Expr,
    budget: &mut SceneBudgetState,
    depth: usize,
    path: Vec<Expr>,
) -> Expr {
    let encoded_bytes = estimated_tree_bytes(label, value);
    if let Err(exhausted) = budget.admit(depth, Some(label), encoded_bytes) {
        return continuation_node(label, exhausted);
    }
    match value {
        Expr::Map(entries) => node(
            "tree",
            vec![
                ("label", Expr::String(label.to_owned())),
                ("open", Expr::Bool(depth == 0)),
                ("aria-expanded", Expr::Bool(depth == 0)),
                ("disclosure-target", Expr::List(path.clone())),
                (
                    "nodes",
                    Expr::List(
                        entries
                            .iter()
                            .map(|(key, child)| {
                                let mut child_path = path.clone();
                                child_path.push(Expr::Vector(vec![sym("k"), key.clone()]));
                                tree_of(&render_value(key), child, budget, depth + 1, child_path)
                            })
                            .collect(),
                    ),
                ),
            ],
        ),
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => node(
            "tree",
            vec![
                ("label", Expr::String(format!("{label} [{}]", items.len()))),
                ("open", Expr::Bool(depth == 0)),
                ("aria-expanded", Expr::Bool(depth == 0)),
                ("disclosure-target", Expr::List(path.clone())),
                (
                    "nodes",
                    Expr::List(
                        items
                            .iter()
                            .enumerate()
                            .map(|(index, child)| {
                                let mut child_path = path.clone();
                                child_path.push(Expr::Vector(vec![
                                    sym("i"),
                                    Expr::String(index.to_string()),
                                ]));
                                tree_of(&format!("[{index}]"), child, budget, depth + 1, child_path)
                            })
                            .collect(),
                    ),
                ),
            ],
        ),
        atom => text_line(format!("{label}: {}", render_value(atom))),
    }
}

fn continuation_node(label: &str, exhausted: SceneBudgetExhausted) -> Expr {
    node(
        "continuation",
        vec![
            ("label", Expr::String(format!("{label}: more not rendered"))),
            ("truncated", Expr::Bool(true)),
            ("reason", sym(exhausted.reason())),
            ("limit", Expr::String(exhausted.limit().to_string())),
        ],
    )
}

/// Region 3: the canonical text. Each SCALAR leaf is an editable text field
/// bound to its OWN field path, so committing an edit sets only that leaf and
/// preserves its siblings (set semantics). A structured value is NOT exposed as
/// a single root-path text field: text is not parsed back into structure here,
/// so editing the whole value as text would clobber it. Structured editing is
/// the structure tree's job; scalar leaves edit in place.
fn canonical_text(value: &Expr) -> Expr {
    let mut children = vec![text_line(render_value(value))];
    match value {
        Expr::Map(entries) => {
            for (key, child) in entries {
                if is_scalar(child) {
                    children.push(editable_leaf(value, key_path(key), child));
                }
            }
        }
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => {
            for (index, item) in items.iter().enumerate() {
                if is_scalar(item) {
                    children.push(editable_leaf(value, index_path(index), item));
                }
            }
        }
        scalar => {
            // A bare scalar IS its own leaf: editing it at the root path sets the
            // whole (scalar) value, which is honest -- there is no structure to
            // clobber.
            children.push(editable_leaf(scalar, Expr::List(Vec::new()), scalar));
        }
    }
    node(
        "box",
        vec![
            ("role", sym("canonical-text")),
            ("children", Expr::List(children)),
        ],
    )
}

/// True when `value` is an atom (no nested structure to edit per-field).
fn is_scalar(value: &Expr) -> bool {
    !matches!(
        value,
        Expr::Map(_) | Expr::List(_) | Expr::Vector(_) | Expr::Set(_)
    )
}

/// The `k`/`i` wire path that scopes an edit to a single map key.
fn key_path(key: &Expr) -> Expr {
    Expr::List(vec![Expr::Vector(vec![sym("k"), key.clone()])])
}

/// The `k`/`i` wire path that scopes an edit to a single sequence index.
fn index_path(index: usize) -> Expr {
    Expr::List(vec![Expr::Vector(vec![
        sym("i"),
        Expr::String(index.to_string()),
    ])])
}

/// An editable text field for one scalar `leaf`, bound to `path` within `root`.
/// The field's `target` is the root value and `path` scopes the edit, so an
/// `edit-field` built from it sets only that leaf.
fn editable_leaf(root: &Expr, path: Expr, leaf: &Expr) -> Expr {
    let mut fields = vec![
        ("input-kind", sym("text")),
        ("value", Expr::String(render_value(leaf))),
        ("value-kind", sym(expr_kind(leaf))),
        ("target", root.clone()),
        ("path", path),
        ("readonly", Expr::Bool(false)),
    ];
    if let Ok(encoded) = sim_codec::encode_portable(CodecId(0), leaf) {
        fields.push(("value-codec", Expr::String(encoded)));
    }
    node("field", fields)
}

/// Region 4: properties and actions as buttons emitting `intent/invoke`.
fn operations_inspector(value: &Expr) -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("operations")),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![
                    action_button("copy", "Copy", value),
                    action_button("edit", "Edit", value),
                ]),
            ),
        ],
    )
}

fn action_button(control: &str, label: &str, value: &Expr) -> Expr {
    node(
        "button",
        vec![
            ("control", sym(control)),
            ("label", Expr::String(label.to_owned())),
            ("target", value.clone()),
        ],
    )
}

fn roundtrip_badge(value: &Expr) -> Expr {
    if exceeds_depth(value, 64) {
        return badge("info", "too deep to round-trip inline");
    }
    let codec = CodecId(0);
    match sim_codec::encode_portable(codec, value) {
        Ok(text) => match sim_codec::decode_portable(codec, &text) {
            Ok(decoded) if &decoded == value => badge("ok", "round-trips"),
            Ok(_) => badge("warn", "round-trip differs"),
            Err(_) => badge("warn", "decode failed"),
        },
        Err(_) => badge("info", "non-data value"),
    }
}

fn exceeds_depth(value: &Expr, max_depth: usize) -> bool {
    let mut stack = vec![(value, 0usize)];
    while let Some((value, depth)) = stack.pop() {
        if depth > max_depth {
            return true;
        }
        match value {
            Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => {
                stack.extend(items.iter().map(|item| (item, depth + 1)));
            }
            Expr::Map(entries) => {
                for (key, value) in entries {
                    stack.push((key, depth + 1));
                    stack.push((value, depth + 1));
                }
            }
            _ => {}
        }
    }
    false
}

/// The four universal regions in increasing-depth order: summary, canonical
/// text, structure tree, operations. Mode-aware rendering takes a prefix.
pub(crate) fn universal_regions(value: &Expr) -> Vec<Expr> {
    vec![
        summary_card(value),
        canonical_text(value),
        structure_tree(value),
        operations_inspector(value),
    ]
}

/// A short human-readable kind name for the value.
pub use sim_value::kind::expr_kind;

fn short_label(value: &Expr) -> String {
    let rendered = render_value(value);
    if rendered.len() <= 48 {
        rendered
    } else {
        format!("{}...", &rendered[..45])
    }
}

/// Render a value as compact, readable text for display.
pub fn render_value(value: &Expr) -> String {
    render_value_bounded(value, 0)
}

fn render_value_bounded(value: &Expr, depth: usize) -> String {
    if depth >= 64 {
        return "...".to_owned();
    }
    match value {
        Expr::Nil => "nil".to_owned(),
        Expr::Bool(flag) => flag.to_string(),
        Expr::Number(number) => number.canonical.clone(),
        Expr::Symbol(symbol) | Expr::Local(symbol) => symbol.as_qualified_str(),
        Expr::String(text) => format!("{text:?}"),
        Expr::Bytes(bytes) => format!("#bytes({})", bytes.len()),
        Expr::List(items) => format!("({})", render_items(items, depth + 1)),
        Expr::Vector(items) => format!("[{}]", render_items(items, depth + 1)),
        Expr::Set(items) => format!("#{{{}}}", render_items(items, depth + 1)),
        Expr::Map(entries) => {
            let body = entries
                .iter()
                .take(128)
                .map(|(key, value)| {
                    format!(
                        "{}: {}",
                        render_value_bounded(key, depth + 1),
                        render_value_bounded(value, depth + 1)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            if entries.len() > 128 {
                format!("{{{body}, ...}}")
            } else {
                format!("{{{body}}}")
            }
        }
        other => format!("<{}>", expr_kind(other)),
    }
}

fn render_items(items: &[Expr], depth: usize) -> String {
    let rendered = items
        .iter()
        .take(128)
        .map(|item| render_value_bounded(item, depth + 1))
        .collect::<Vec<_>>()
        .join(" ");
    if items.len() > 128 {
        format!("{rendered} ...")
    } else {
        rendered
    }
}

fn estimated_tree_bytes(label: &str, value: &Expr) -> usize {
    let body = match value {
        Expr::Map(entries) => entries.len().saturating_mul(16),
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => items.len().saturating_mul(8),
        atom => render_value(atom).len(),
    };
    label.len().saturating_add(body)
}
