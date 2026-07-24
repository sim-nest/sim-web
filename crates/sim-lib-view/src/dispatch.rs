//! Shape-based lens dispatch.
//!
//! Choosing a lens for a value is overload selection, which is exactly what the
//! kernel `Shape` matcher already does. The dispatcher reuses that matcher and
//! the documented resolution order; it is implemented once here and never
//! reimplemented per domain. Resolution order:
//!
//! 1. explicit operator choice;
//! 2. saved workspace preference;
//! 3. best Shape match (most specific wins; ties by quality then cost);
//! 4. class match fallback;
//! 5. the universal default (always matches, lowest quality).
//!
//! Every candidate must pass capability filtering; a denied lens is skipped and
//! resolution falls through, ending at the read-only universal default.

use std::collections::BTreeMap;
use std::sync::Arc;

use sim_kernel::{CapabilityName, Cx, Error, Expr, Result, Symbol};

use crate::codec::SurfaceCodec;
use crate::contract::{Lens, LensKind};

/// The context a dispatch runs in: operator choice, saved preference, active
/// mode, the value's class, and the capability predicate.
pub struct DispatchContext<'a> {
    /// An explicit lens choice (from `intent/set-lens`).
    pub explicit: Option<Symbol>,
    /// A saved workspace preference for this resource.
    pub preference: Option<Symbol>,
    /// The active experience mode, if any.
    pub active_mode: Option<Symbol>,
    /// The value's class symbol, for the class-match fallback.
    pub value_class: Option<Symbol>,
    /// Returns whether a capability is granted to the operator.
    pub granted: &'a dyn Fn(&CapabilityName) -> bool,
}

impl<'a> DispatchContext<'a> {
    /// A context that grants every capability and has no preferences.
    pub fn permissive(grant_all: &'a dyn Fn(&CapabilityName) -> bool) -> Self {
        Self {
            explicit: None,
            preference: None,
            active_mode: None,
            value_class: None,
            granted: grant_all,
        }
    }
}

/// Why the dispatcher chose a lens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatchReason {
    /// Selected by explicit operator choice.
    Explicit,
    /// Selected by saved workspace preference.
    Preference,
    /// Selected as the best Shape match, with the winning match score.
    ShapeMatch(i32),
    /// Selected by class-match fallback.
    ClassMatch,
    /// Selected as the universal default.
    UniversalDefault,
}

/// The outcome of a successful dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchOutcome {
    /// The chosen lens id.
    pub lens_id: Symbol,
    /// Why it was chosen.
    pub reason: DispatchReason,
}

/// A registry of lenses with a single shared dispatcher.
#[derive(Default)]
pub struct LensRegistry {
    lenses: Vec<Lens>,
    surface_codecs: BTreeMap<Symbol, Arc<dyn SurfaceCodec>>,
}

impl LensRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a lens (last registration of an id wins on exact ties).
    pub fn register(&mut self, lens: Lens) {
        self.lenses.push(lens);
    }

    /// Register a reversible surface codec.
    pub fn register_surface_codec(&mut self, id: Symbol, codec: Arc<dyn SurfaceCodec>) {
        self.surface_codecs.insert(id, codec);
    }

    /// Look up a reversible surface codec by id.
    pub fn surface_codec(&self, id: &Symbol) -> Option<Arc<dyn SurfaceCodec>> {
        self.surface_codecs.get(id).cloned()
    }

    /// Look up a lens by id.
    pub fn get(&self, id: &Symbol) -> Option<&Lens> {
        self.lenses.iter().find(|lens| &lens.meta.id == id)
    }

    /// All registered lenses.
    pub fn lenses(&self) -> &[Lens] {
        &self.lenses
    }

    /// Dispatch a `View` lens for `target`.
    pub fn dispatch_view(
        &self,
        cx: &mut Cx,
        target: &Expr,
        ctx: &DispatchContext,
    ) -> Result<DispatchOutcome> {
        self.dispatch(cx, LensKind::View, target, ctx)
    }

    /// Dispatch an `Editor` lens for `target`.
    pub fn dispatch_editor(
        &self,
        cx: &mut Cx,
        target: &Expr,
        ctx: &DispatchContext,
    ) -> Result<DispatchOutcome> {
        self.dispatch(cx, LensKind::Editor, target, ctx)
    }

    /// Resolve a lens of `kind` for `target` per the documented order.
    pub fn dispatch(
        &self,
        cx: &mut Cx,
        kind: LensKind,
        target: &Expr,
        ctx: &DispatchContext,
    ) -> Result<DispatchOutcome> {
        // 1. explicit operator choice.
        if let Some(outcome) = self.pick_named(&ctx.explicit, kind, ctx, DispatchReason::Explicit) {
            return Ok(outcome);
        }
        // 2. saved workspace preference.
        if let Some(outcome) =
            self.pick_named(&ctx.preference, kind, ctx, DispatchReason::Preference)
        {
            return Ok(outcome);
        }
        // 3. best Shape match (most specific wins; ties by quality then cost).
        if let Some(outcome) = self.pick_shape_match(cx, kind, target, ctx)? {
            return Ok(outcome);
        }
        // 4. class match fallback.
        if let Some(outcome) = self.pick_class_match(kind, ctx) {
            return Ok(outcome);
        }
        // 5. universal default.
        if let Some(lens) = self.lenses.iter().find(|lens| {
            lens.meta.kind == kind && lens.meta.universal_default && self.allowed(lens, ctx)
        }) {
            return Ok(DispatchOutcome {
                lens_id: lens.meta.id.clone(),
                reason: DispatchReason::UniversalDefault,
            });
        }
        Err(Error::HostError(format!(
            "no {kind:?} lens available for the value (not even a universal default)"
        )))
    }

    pub(crate) fn allowed(&self, lens: &Lens, ctx: &DispatchContext) -> bool {
        lens.meta
            .required_capabilities
            .iter()
            .all(|capability| (ctx.granted)(capability))
    }

    fn pick_named(
        &self,
        id: &Option<Symbol>,
        kind: LensKind,
        ctx: &DispatchContext,
        reason: DispatchReason,
    ) -> Option<DispatchOutcome> {
        let id = id.as_ref()?;
        let lens = self.get(id)?;
        if lens.meta.kind == kind && self.allowed(lens, ctx) {
            Some(DispatchOutcome {
                lens_id: id.clone(),
                reason,
            })
        } else {
            None
        }
    }

    fn pick_shape_match(
        &self,
        cx: &mut Cx,
        kind: LensKind,
        target: &Expr,
        ctx: &DispatchContext,
    ) -> Result<Option<DispatchOutcome>> {
        let mut best: Option<(i32, i32, i32, Symbol)> = None;
        for lens in &self.lenses {
            if lens.meta.kind != kind || lens.meta.universal_default || !self.allowed(lens, ctx) {
                continue;
            }
            let Some(score) = best_shape_score(cx, lens, target)? else {
                continue;
            };
            // Rank by (score, quality, -cost); first registered wins exact ties.
            let candidate = (score, lens.meta.quality, -lens.meta.cost);
            let better = match &best {
                Some((bs, bq, bc, _)) => candidate > (*bs, *bq, *bc),
                None => true,
            };
            if better {
                best = Some((candidate.0, candidate.1, candidate.2, lens.meta.id.clone()));
            }
        }
        Ok(best.map(|(score, _, _, lens_id)| DispatchOutcome {
            lens_id,
            reason: DispatchReason::ShapeMatch(score),
        }))
    }

    fn pick_class_match(&self, kind: LensKind, ctx: &DispatchContext) -> Option<DispatchOutcome> {
        let class = ctx.value_class.as_ref()?;
        let mut best: Option<(i32, i32, Symbol)> = None;
        for lens in &self.lenses {
            if lens.meta.kind != kind
                || lens.meta.universal_default
                || !self.allowed(lens, ctx)
                || !lens.meta.claimed_classes.contains(class)
            {
                continue;
            }
            let candidate = (lens.meta.quality, -lens.meta.cost);
            let better = match &best {
                Some((bq, bc, _)) => candidate > (*bq, *bc),
                None => true,
            };
            if better {
                best = Some((candidate.0, candidate.1, lens.meta.id.clone()));
            }
        }
        best.map(|(_, _, lens_id)| DispatchOutcome {
            lens_id,
            reason: DispatchReason::ClassMatch,
        })
    }
}

/// The best accepted Shape match score among a lens's claimed Shapes, if any.
pub(crate) fn best_shape_score(cx: &mut Cx, lens: &Lens, target: &Expr) -> Result<Option<i32>> {
    let mut best: Option<i32> = None;
    for shape_value in &lens.meta.claimed_shapes {
        let Some(shape) = shape_value.object().as_shape() else {
            continue;
        };
        let matched = shape.check_expr(cx, target)?;
        if matched.accepted {
            let score = matched.score.value();
            best = Some(best.map_or(score, |current| current.max(score)));
        }
    }
    Ok(best)
}
