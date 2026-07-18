//! The universal default editor: edit any value over one draft.
//!
//! The editor renders a draft in two real projections -- a readable `text`
//! rendering and the codec-portable `raw` form (see [`EDIT_MODES`]). It
//! validates before commit, anchors errors to the edited field, allows
//! cancel/revert, preserves unknown fields when editing open maps (set
//! semantics keep sibling keys), and refuses to commit a readonly value.
//!
//! Note: earlier scaffolding advertised four "synchronized" modes
//! (form/tree/text/raw), but form, tree, and text all rendered identically;
//! only the two distinct projections below are advertised, so the mode list
//! matches what is actually implemented.

use std::sync::Arc;

use sim_kernel::{Cx, Diagnostic, Error, Expr, ExprKind, NumberLiteral, Result, ShapeRef, Symbol};
use sim_lib_intent::{field, intent_kind_of};
use sim_shape::{ExprKindShape, shape_value};
use sim_value::path::{Path, PathError, get, set_at};

use crate::contract::{Draft, Editor, Operation};

/// The real edit-mode projections over one draft: a readable `text` rendering
/// and the codec-portable `raw` form. These are the only two distinct
/// projections [`render_draft`] produces, so only they are advertised.
pub const EDIT_MODES: &[&str] = &["text", "raw"];

/// The universal default editor.
pub struct UniversalEditor {
    readonly: bool,
}

impl UniversalEditor {
    /// A writable universal editor.
    pub fn writable() -> Self {
        Self { readonly: false }
    }

    /// A read-only universal editor: it renders but never commits.
    pub fn readonly() -> Self {
        Self { readonly: true }
    }
}

impl Editor for UniversalEditor {
    fn decode(&self, _cx: &mut Cx, value: &Expr, intent: &Expr) -> Result<Draft> {
        let Some(kind) = intent_kind_of(intent) else {
            return Err(Error::HostError("editor input is not an Intent".to_owned()));
        };
        match &*kind.name {
            "edit-field" => self.edit_field(value, intent),
            "commit" => Ok(Draft::clean(value.clone(), value.clone())),
            "cancel" => {
                // Revert: discard any pending edit, proposing the base unchanged.
                Ok(Draft::clean(value.clone(), value.clone()))
            }
            other => Ok(Draft::rejected(
                value.clone(),
                Diagnostic::error(format!("universal editor does not handle intent '{other}'")),
            )),
        }
    }

    fn commit(&self, _cx: &mut Cx, draft: &Draft) -> Result<Operation> {
        if !draft.committable {
            return Err(Error::HostError(
                "draft is not committable; resolve diagnostics first".to_owned(),
            ));
        }
        // The operation realizes by setting the resource to the proposed value.
        Ok(Operation::new(Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("op")),
                Expr::Symbol(Symbol::new("set-value")),
            ),
            (Expr::Symbol(Symbol::new("value")), draft.proposed.clone()),
        ]))
        .with_result_shape(expr_kind_shape(&draft.base)))
    }
}

fn expr_kind_shape(expr: &Expr) -> ShapeRef {
    let kind = match expr {
        Expr::Nil => ExprKind::Nil,
        Expr::Bool(_) => ExprKind::Bool,
        Expr::Number(_) => ExprKind::Number,
        Expr::Symbol(_) | Expr::Local(_) => ExprKind::Symbol,
        Expr::String(_) => ExprKind::String,
        Expr::Bytes(_) => ExprKind::Bytes,
        Expr::List(_) => ExprKind::List,
        Expr::Vector(_) => ExprKind::Vector,
        Expr::Map(_) => ExprKind::Map,
        Expr::Set(_) => ExprKind::Set,
        Expr::Call { .. } => ExprKind::Call,
        Expr::Infix { .. } => ExprKind::Infix,
        Expr::Prefix { .. } => ExprKind::Prefix,
        Expr::Postfix { .. } => ExprKind::Postfix,
        Expr::Block(_) => ExprKind::Block,
        Expr::Quote { .. } => ExprKind::Quote,
        Expr::Annotated { .. } => ExprKind::Annotated,
        Expr::Extension { .. } => ExprKind::Extension,
    };
    shape_value(
        Symbol::qualified("core", format!("{kind:?}")),
        Arc::new(ExprKindShape::new(kind)),
    )
}

impl UniversalEditor {
    fn edit_field(&self, value: &Expr, intent: &Expr) -> Result<Draft> {
        if self.readonly {
            return Ok(Draft::rejected(
                value.clone(),
                Diagnostic::error("value is read-only and cannot be edited"),
            ));
        }
        let Some(path_expr @ Expr::List(_)) = field(intent, "path") else {
            return Ok(Draft::rejected(
                value.clone(),
                Diagnostic::error("edit-field is missing a list 'path'"),
            ));
        };
        let Some(new_value) = field(intent, "value") else {
            return Ok(Draft::rejected(
                value.clone(),
                Diagnostic::error("edit-field is missing a 'value'"),
            ));
        };
        let path = match Path::from_expr(path_expr) {
            Ok(path) => path,
            Err(error) => {
                return Ok(Draft::rejected(
                    value.clone(),
                    path_error_diagnostic(path_expr, error),
                ));
            }
        };
        let replacement = match get(value, &path) {
            Some(current) => match coerce_edit_value(current, new_value) {
                Ok(value) => value,
                Err(message) => {
                    return Ok(Draft::rejected(
                        value.clone(),
                        field_shape_diagnostic(path_expr, message),
                    ));
                }
            },
            None => new_value.clone(),
        };
        match set_at(value, &path, replacement) {
            Ok(proposed) => Ok(Draft::clean(value.clone(), proposed)),
            Err(error) => Ok(Draft::rejected(
                value.clone(),
                path_error_diagnostic(path_expr, error),
            )),
        }
    }
}

fn coerce_edit_value(current: &Expr, submitted: &Expr) -> core::result::Result<Expr, String> {
    if !matches!(submitted, Expr::String(_)) {
        return Ok(submitted.clone());
    }
    match current {
        Expr::String(_) => match submitted {
            Expr::String(_) => Ok(submitted.clone()),
            other => Err(format!(
                "string field received {}",
                crate::universal_view::expr_kind(other)
            )),
        },
        Expr::Bool(_) => coerce_bool(submitted),
        Expr::Number(number) => coerce_number(number, submitted),
        Expr::Symbol(symbol) => coerce_symbol(symbol, submitted).map(Expr::Symbol),
        Expr::Local(symbol) => coerce_symbol(symbol, submitted).map(Expr::Local),
        Expr::Nil => coerce_nil(submitted),
        Expr::Bytes(_) => match submitted {
            Expr::Bytes(_) => Ok(submitted.clone()),
            other => Err(format!(
                "bytes field received {}",
                crate::universal_view::expr_kind(other)
            )),
        },
        Expr::Map(_)
        | Expr::List(_)
        | Expr::Vector(_)
        | Expr::Set(_)
        | Expr::Call { .. }
        | Expr::Infix { .. }
        | Expr::Prefix { .. }
        | Expr::Postfix { .. }
        | Expr::Block(_)
        | Expr::Quote { .. }
        | Expr::Annotated { .. }
        | Expr::Extension { .. } => Ok(submitted.clone()),
    }
}

fn coerce_bool(submitted: &Expr) -> core::result::Result<Expr, String> {
    match submitted {
        Expr::Bool(_) => Ok(submitted.clone()),
        Expr::String(text) => match text.trim() {
            "true" => Ok(Expr::Bool(true)),
            "false" => Ok(Expr::Bool(false)),
            _ => Err(format!("bool field cannot parse {text:?}")),
        },
        other => Err(format!(
            "bool field received {}",
            crate::universal_view::expr_kind(other)
        )),
    }
}

fn coerce_number(current: &NumberLiteral, submitted: &Expr) -> core::result::Result<Expr, String> {
    match submitted {
        Expr::Number(_) => Ok(submitted.clone()),
        Expr::String(text) => {
            let canonical = text.trim();
            validate_number_text(&current.canonical, canonical)?;
            Ok(Expr::Number(NumberLiteral {
                domain: current.domain.clone(),
                canonical: canonical.to_owned(),
            }))
        }
        other => Err(format!(
            "number field received {}",
            crate::universal_view::expr_kind(other)
        )),
    }
}

fn validate_number_text(old: &str, new: &str) -> core::result::Result<(), String> {
    if new.is_empty() {
        return Err("number field cannot be empty".to_owned());
    }
    if old.parse::<i128>().is_ok() {
        new.parse::<i128>()
            .map(|_| ())
            .map_err(|_| format!("integer field cannot parse {new:?}"))
    } else if old.parse::<f64>().is_ok() {
        match new.parse::<f64>() {
            Ok(value) if value.is_finite() => Ok(()),
            _ => Err(format!("number field cannot parse {new:?}")),
        }
    } else if new.chars().any(char::is_control) {
        Err("number field cannot contain control characters".to_owned())
    } else {
        Ok(())
    }
}

fn coerce_symbol(current: &Symbol, submitted: &Expr) -> core::result::Result<Symbol, String> {
    match submitted {
        Expr::Symbol(symbol) | Expr::Local(symbol) => Ok(symbol.clone()),
        Expr::String(text) => parse_symbol_like(current, text),
        other => Err(format!(
            "symbol field received {}",
            crate::universal_view::expr_kind(other)
        )),
    }
}

fn parse_symbol_like(current: &Symbol, text: &str) -> core::result::Result<Symbol, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("symbol field cannot be empty".to_owned());
    }
    if current.namespace.is_some()
        && let Some((namespace, name)) = text.split_once('/')
    {
        if namespace.is_empty() || name.is_empty() || name.contains('/') {
            return Err(format!("symbol field cannot parse {text:?}"));
        }
        return Ok(Symbol::qualified(namespace.to_owned(), name.to_owned()));
    }
    Symbol::checked(text.to_owned()).map_err(|error| error.to_string())
}

fn coerce_nil(submitted: &Expr) -> core::result::Result<Expr, String> {
    match submitted {
        Expr::Nil => Ok(Expr::Nil),
        Expr::String(text) if text.trim() == "nil" => Ok(Expr::Nil),
        other => Err(format!(
            "nil field received {}",
            crate::universal_view::expr_kind(other)
        )),
    }
}

/// Render a draft in one of the real edit modes (`text` readable, `raw`
/// codec-portable; see [`EDIT_MODES`]). Both are views over the same
/// `draft.proposed`, so switching mode never changes the draft. Any unknown
/// mode renders the readable form.
pub fn render_draft(draft: &Draft, mode: &str) -> Result<Expr> {
    let proposed = &draft.proposed;
    // `raw` shows the codec-portable encoding; every other mode renders the
    // readable form.
    let body = match mode {
        "raw" => sim_codec::encode_portable(sim_kernel::CodecId(0), proposed)
            .unwrap_or_else(|_| crate::universal_view::render_value(proposed)),
        _ => crate::universal_view::render_value(proposed),
    };
    Ok(sim_lib_scene::node(
        "box",
        vec![
            ("role", Expr::Symbol(Symbol::new("edit"))),
            ("mode", Expr::Symbol(Symbol::new(mode))),
            (
                "children",
                Expr::List(vec![
                    sim_lib_scene::node(
                        "field",
                        vec![
                            ("input-kind", Expr::Symbol(Symbol::new("text"))),
                            ("value", Expr::String(body)),
                            ("readonly", Expr::Bool(false)),
                        ],
                    ),
                    committable_badge(draft),
                ]),
            ),
        ],
    ))
}

fn committable_badge(draft: &Draft) -> Expr {
    if draft.committable {
        sim_lib_scene::node(
            "badge",
            vec![
                ("status", Expr::Symbol(Symbol::new("ok"))),
                ("label", Expr::String("ready to commit".to_owned())),
            ],
        )
    } else {
        let message = draft
            .diagnostics
            .first()
            .map(|diagnostic| diagnostic.message.clone())
            .unwrap_or_else(|| "not committable".to_owned());
        sim_lib_scene::node(
            "badge",
            vec![
                ("status", Expr::Symbol(Symbol::new("error"))),
                ("label", Expr::String(message)),
            ],
        )
    }
}

/// Turn a `sim_value::path` failure into a field-anchored diagnostic naming the
/// edit path. The set itself is the shared `sim_value::path::set_at` primitive.
fn path_error_diagnostic(path: &Expr, error: PathError) -> Diagnostic {
    Diagnostic::error(format!(
        "edit rejected at path {}: {error:?}",
        crate::universal_view::render_value(path)
    ))
}

fn field_shape_diagnostic(path: &Expr, message: String) -> Diagnostic {
    Diagnostic::error(format!(
        "edit rejected at path {}: {message}",
        crate::universal_view::render_value(path)
    ))
}
