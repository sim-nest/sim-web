//! Lens render and propose helpers.
//!
//! These tie the contracts to the scene and intent domains: a view's output
//! must validate as a Scene, and an editor only sees an Intent that has already
//! passed `codec:intent` validation. Both fail closed: an invalid scene or
//! intent is a diagnostic, never a silent partial result.

use sim_kernel::{Cx, Error, Expr, Result, Symbol};

use crate::contract::{Draft, Operation};
use crate::dispatch::LensRegistry;

impl LensRegistry {
    /// Render `value` through the named view lens, validating the emitted Scene.
    pub fn render(&self, cx: &mut Cx, lens_id: &Symbol, value: &Expr) -> Result<Expr> {
        let lens = self
            .get(lens_id)
            .ok_or_else(|| Error::HostError(format!("unknown lens {lens_id}")))?;
        let view = lens
            .view
            .as_ref()
            .ok_or_else(|| Error::HostError(format!("lens {lens_id} is not a view")))?;
        let scene = view.encode(cx, value)?;
        sim_lib_scene::validate_scene(&scene).map_err(|error| {
            Error::HostError(format!("view {lens_id} produced an invalid scene: {error}"))
        })?;
        Ok(scene)
    }

    /// Fold an Intent into a draft through the named editor lens, after the
    /// Intent passes structural validation.
    pub fn propose(
        &self,
        cx: &mut Cx,
        lens_id: &Symbol,
        value: &Expr,
        intent: &Expr,
    ) -> Result<Draft> {
        sim_lib_intent::validate_intent(intent)
            .map_err(|error| Error::HostError(format!("invalid intent: {error}")))?;
        let editor = self.editor_of(lens_id)?;
        editor.decode(cx, value, intent)
    }

    /// Commit a committable draft through the named editor lens.
    pub fn commit(&self, cx: &mut Cx, lens_id: &Symbol, draft: &Draft) -> Result<Operation> {
        if !draft.committable {
            return Err(Error::HostError(format!(
                "draft is not committable ({} diagnostic(s))",
                draft.diagnostics.len()
            )));
        }
        self.editor_of(lens_id)?.commit(cx, draft)
    }

    fn editor_of(&self, lens_id: &Symbol) -> Result<std::sync::Arc<dyn crate::contract::Editor>> {
        let lens = self
            .get(lens_id)
            .ok_or_else(|| Error::HostError(format!("unknown lens {lens_id}")))?;
        lens.editor
            .clone()
            .ok_or_else(|| Error::HostError(format!("lens {lens_id} is not an editor")))
    }
}
