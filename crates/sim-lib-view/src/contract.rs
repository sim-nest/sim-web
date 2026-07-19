//! View/editor contracts and their open metadata record.
//!
//! A **view** is an encoder in the codec sense: `Value -> Scene`. An **editor**
//! is a decoder: `(Value, Intent) -> Draft`, then `Draft -> operation`. A
//! **lens** pairs a view with an optional editor and carries an
//! `ExportRecord`-style metadata record ([`LensMeta`]) describing what it claims
//! and what it costs. The metadata is an open record, not a closed kernel enum:
//! new lens kinds and fields ride along as data rather than forcing kernel
//! changes.

use std::sync::Arc;

use sim_kernel::{CapabilityName, Cx, Diagnostic, Expr, Result, ShapeRef, Symbol};

/// The role a lens plays. A lens of a given kind is only considered when a pane
/// asks for that kind (a pane showing a value asks for `View`; an editing pane
/// asks for `Editor`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LensKind {
    /// Encodes a value into a Scene for display.
    View,
    /// Decodes Intents into checked operations.
    Editor,
    /// A read-only detail panel over a value.
    Inspector,
    /// A floating layer (tooltip, popover, confirmation sheet).
    Overlay,
    /// A single invokable action surfaced as a control.
    Action,
}

/// Open metadata describing a registered lens. Modelled on `ExportRecord`: a
/// data record the dispatcher reads, never a closed kernel enum plus a parallel
/// registry map.
#[derive(Clone)]
pub struct LensMeta {
    /// Stable lens id (for example `view:agent-topology`).
    pub id: Symbol,
    /// The role this lens plays.
    pub kind: LensKind,
    /// Shapes this lens claims; a lens matches a value when one accepts it.
    pub claimed_shapes: Vec<ShapeRef>,
    /// Class symbols this lens claims as a fallback when no Shape matches.
    pub claimed_classes: Vec<Symbol>,
    /// Declared quality; higher wins ties among equally specific matches.
    pub quality: i32,
    /// Declared cost; lower wins ties after quality.
    pub cost: i32,
    /// Capabilities the operator must hold for this lens to be eligible.
    pub required_capabilities: Vec<CapabilityName>,
    /// Experience modes this lens prefers for mode-aware ranking.
    pub preferred_modes: Vec<Symbol>,
    /// Whether this is the always-matching universal default of its kind.
    pub universal_default: bool,
}

impl LensMeta {
    /// Build a minimal lens metadata record with no claims and zero quality and
    /// cost. Use the builder methods to fill it in.
    pub fn new(id: Symbol, kind: LensKind) -> Self {
        Self {
            id,
            kind,
            claimed_shapes: Vec::new(),
            claimed_classes: Vec::new(),
            quality: 0,
            cost: 0,
            required_capabilities: Vec::new(),
            preferred_modes: Vec::new(),
            universal_default: false,
        }
    }

    /// Claim a Shape (a shape `Value`).
    pub fn claiming_shape(mut self, shape: ShapeRef) -> Self {
        self.claimed_shapes.push(shape);
        self
    }

    /// Claim a class symbol as a fallback match.
    pub fn claiming_class(mut self, class: Symbol) -> Self {
        self.claimed_classes.push(class);
        self
    }

    /// Set quality and cost.
    pub fn with_quality_cost(mut self, quality: i32, cost: i32) -> Self {
        self.quality = quality;
        self.cost = cost;
        self
    }

    /// Require a capability.
    pub fn requiring(mut self, capability: CapabilityName) -> Self {
        self.required_capabilities.push(capability);
        self
    }

    /// Prefer an experience mode.
    pub fn preferring_mode(mut self, mode: Symbol) -> Self {
        self.preferred_modes.push(mode);
        self
    }

    /// Mark this lens as the universal default of its kind.
    pub fn as_universal_default(mut self) -> Self {
        self.universal_default = true;
        self
    }
}

/// A view encoder: `Value -> Scene`. Views are pure: the same value and options
/// yield the same Scene.
pub trait View: Send + Sync {
    /// Encode the value (in `Expr` form) into a Scene value.
    fn encode(&self, cx: &mut Cx, value: &Expr) -> Result<Expr>;
}

/// An editor decoder: `(Value, Intent) -> Draft`, then `Draft -> operation`.
pub trait Editor: Send + Sync {
    /// Fold an Intent into a pending draft over the value.
    fn decode(&self, cx: &mut Cx, value: &Expr, intent: &Expr) -> Result<Draft>;
    /// Turn a committable draft into a checked operation.
    fn commit(&self, cx: &mut Cx, draft: &Draft) -> Result<Operation>;
}

/// A pending edit over a value: the base, the proposed value, whether it may be
/// committed, and any field-anchored diagnostics.
#[derive(Clone, Debug)]
pub struct Draft {
    /// The value being edited, before this draft.
    pub base: Expr,
    /// The proposed value if the draft commits.
    pub proposed: Expr,
    /// Whether the draft currently passes validation.
    pub committable: bool,
    /// Field-anchored diagnostics; non-empty implies not committable.
    pub diagnostics: Vec<Diagnostic>,
}

impl Draft {
    /// A clean, committable draft proposing `proposed` over `base`.
    pub fn clean(base: Expr, proposed: Expr) -> Self {
        Self {
            base,
            proposed,
            committable: true,
            diagnostics: Vec::new(),
        }
    }

    /// A rejected draft anchored to a diagnostic; the base is preserved.
    pub fn rejected(base: Expr, diagnostic: Diagnostic) -> Self {
        Self {
            proposed: base.clone(),
            base,
            committable: false,
            diagnostics: vec![diagnostic],
        }
    }
}

/// A checked operation produced by an editor commit, ready to be submitted
/// through `realize`.
#[derive(Clone, Debug)]
pub struct Operation {
    /// The checked operation form to realize.
    pub form: Expr,
    /// Optional shape the realized result must satisfy before it is accepted.
    pub result_shape: Option<ShapeRef>,
    /// Capabilities the realization target must hold to answer the operation.
    pub required_capabilities: Vec<CapabilityName>,
}

impl Operation {
    /// Build an operation with no additional authority metadata.
    pub fn new(form: Expr) -> Self {
        Self {
            form,
            result_shape: None,
            required_capabilities: Vec::new(),
        }
    }

    /// Attach the expected shape of the realized result.
    pub fn with_result_shape(mut self, shape: ShapeRef) -> Self {
        self.result_shape = Some(shape);
        self
    }

    /// Attach one capability required by the realization target.
    pub fn requiring(mut self, capability: CapabilityName) -> Self {
        self.required_capabilities.push(capability);
        self
    }
}

/// A registered lens: its metadata plus optional view and editor objects.
#[derive(Clone)]
pub struct Lens {
    /// The lens metadata the dispatcher reads.
    pub meta: LensMeta,
    /// The view encoder, if this lens renders.
    pub view: Option<Arc<dyn View>>,
    /// The editor decoder, if this lens edits.
    pub editor: Option<Arc<dyn Editor>>,
}

impl Lens {
    /// A metadata-only lens (no view or editor object yet).
    pub fn metadata_only(meta: LensMeta) -> Self {
        Self {
            meta,
            view: None,
            editor: None,
        }
    }

    /// A view lens backed by `view`.
    pub fn view(meta: LensMeta, view: Arc<dyn View>) -> Self {
        Self {
            meta,
            view: Some(view),
            editor: None,
        }
    }

    /// An editor lens backed by `editor`.
    pub fn editor(meta: LensMeta, editor: Arc<dyn Editor>) -> Self {
        Self {
            meta,
            view: None,
            editor: Some(editor),
        }
    }
}
