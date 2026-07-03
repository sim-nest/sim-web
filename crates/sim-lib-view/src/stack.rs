//! The lens stack: the ordered set of lenses available for a value.
//!
//! A value has many peer lenses. The stack is the dispatcher's full ranking
//! rather than just its winner: every eligible lens for a value, best first,
//! ending at the universal default. The operator flips between entries with
//! `intent/set-lens` (see [`crate::set_lens`]); switching never touches the
//! value.

use sim_kernel::{Cx, Expr, Result, Symbol};

use crate::contract::LensKind;
use crate::dispatch::{DispatchContext, DispatchReason, LensRegistry, best_shape_score};

/// One entry in a value's lens stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LensStackEntry {
    /// The lens id.
    pub lens_id: Symbol,
    /// Why and how strongly it matched.
    pub reason: DispatchReason,
}

impl LensRegistry {
    /// Build the ordered lens stack of `kind` for `target`: every eligible lens
    /// best first, with the universal default last. Capability-denied lenses are
    /// excluded.
    pub fn lens_stack(
        &self,
        cx: &mut Cx,
        kind: LensKind,
        target: &Expr,
        ctx: &DispatchContext,
    ) -> Result<Vec<LensStackEntry>> {
        // (tier, score, quality, -cost, id, reason). Higher tuple ranks first.
        let mut ranked: Vec<(i32, i32, i32, i32, Symbol, DispatchReason)> = Vec::new();
        for lens in self.lenses() {
            if lens.meta.kind != kind || !self.allowed(lens, ctx) {
                continue;
            }
            let meta = &lens.meta;
            if meta.universal_default {
                ranked.push((
                    0,
                    0,
                    meta.quality,
                    -meta.cost,
                    meta.id.clone(),
                    DispatchReason::UniversalDefault,
                ));
            } else if let Some(score) = best_shape_score(cx, lens, target)? {
                ranked.push((
                    2,
                    score,
                    meta.quality,
                    -meta.cost,
                    meta.id.clone(),
                    DispatchReason::ShapeMatch(score),
                ));
            } else if class_matches(meta, ctx) {
                ranked.push((
                    1,
                    0,
                    meta.quality,
                    -meta.cost,
                    meta.id.clone(),
                    DispatchReason::ClassMatch,
                ));
            }
        }
        // Sort best first; the stable sort keeps registration order on ties.
        ranked.sort_by_key(|entry| std::cmp::Reverse((entry.0, entry.1, entry.2, entry.3)));
        Ok(ranked
            .into_iter()
            .map(|(_, _, _, _, lens_id, reason)| LensStackEntry { lens_id, reason })
            .collect())
    }
}

fn class_matches(meta: &crate::contract::LensMeta, ctx: &DispatchContext) -> bool {
    match &ctx.value_class {
        Some(class) => meta.claimed_classes.contains(class),
        None => false,
    }
}
