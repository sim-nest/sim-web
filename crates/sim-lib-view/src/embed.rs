//! Scene embedding: one lens hosting another lens's Scene.
//!
//! Composition is a `scene/embed` node wrapping a nested Scene produced by
//! another lens. A host lens can therefore render a value through a different
//! lens and place the result inside its own Scene without flattening or
//! duplicating it.

use sim_kernel::{Cx, Expr, Result, Symbol};
use sim_lib_scene::node;

use crate::dispatch::LensRegistry;

/// Wrap an already-rendered `inner` Scene in a `scene/embed` node, recording the
/// lens that produced it.
pub fn embed_scene(lens_id: &Symbol, inner: Expr) -> Expr {
    node(
        "embed",
        vec![("lens", Expr::Symbol(lens_id.clone())), ("scene", inner)],
    )
}

impl LensRegistry {
    /// Render `value` through the named view lens and return it wrapped in a
    /// `scene/embed` node, ready to nest inside a host lens's Scene.
    pub fn render_embedded(&self, cx: &mut Cx, lens_id: &Symbol, value: &Expr) -> Result<Expr> {
        let inner = self.render(cx, lens_id, value)?;
        Ok(embed_scene(lens_id, inner))
    }
}
