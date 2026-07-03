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

use sim_kernel::{Cx, Diagnostic, Error, Expr, Result, Symbol};
use sim_lib_intent::{field, intent_kind_of};
use sim_value::path::{Path, PathError, set_at};

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
        Ok(Operation {
            form: Expr::Map(vec![
                (
                    Expr::Symbol(Symbol::new("op")),
                    Expr::Symbol(Symbol::new("set-value")),
                ),
                (Expr::Symbol(Symbol::new("value")), draft.proposed.clone()),
            ]),
        })
    }
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
        match set_at(value, &path, new_value.clone()) {
            Ok(proposed) => Ok(Draft::clean(value.clone(), proposed)),
            Err(error) => Ok(Draft::rejected(
                value.clone(),
                path_error_diagnostic(path_expr, error),
            )),
        }
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
                            ("kind", Expr::Symbol(Symbol::new("text"))),
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
