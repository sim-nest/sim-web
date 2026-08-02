//! Ranked `SurfaceCodec` registration and reversible operation compilation.

use std::sync::Arc;

use sim_kernel::{CapabilityName, Cx, Diagnostic, Error, Expr, Result, ShapeRef, Symbol};
use sim_lib_interference_runtime::{
    install_interference_records, projection_shape_symbol, study_shape_symbol,
};
use sim_lib_view::{
    Draft, Lens, LensKind, LensMeta, LensRegistry, Operation, SurfaceCaps, SurfaceCodec,
};

use crate::intent::{self, EditClass};

/// Stable registry id for the interference study surface.
pub const INTERFERENCE_SURFACE_CODEC_ID: &str = "surface:interference";

/// Authority required to realize a projection edit.
pub const INTERFERENCE_PROJECT_CAPABILITY: &str = "interference/project";

/// Authority required to realize a model edit that performs a new solve.
pub const INTERFERENCE_SOLVE_CAPABILITY: &str = "interference/solve";

/// Absolute number of pre-projected animation frames accepted by one request.
pub const MAX_ANIMATION_FRAMES: usize = 120;

/// Returns the surface registry symbol.
pub fn surface_interference_codec_symbol() -> Symbol {
    Symbol::new(INTERFERENCE_SURFACE_CODEC_ID)
}

/// Installs the published interference record Shapes and registers one ranked
/// surface codec claiming exactly `interference/Study`.
pub fn register_interference_surface(cx: &mut Cx, registry: &mut LensRegistry) -> Result<()> {
    install_interference_records(cx)?;
    let study_shape = registered_shape(cx, study_shape_symbol())?;
    let id = surface_interference_codec_symbol();
    registry.register(Lens::metadata_only(
        LensMeta::new(id.clone(), LensKind::View)
            .claiming_shape(study_shape)
            .with_quality_cost(240, 24),
    ));
    registry.register_surface_codec(id, Arc::new(InterferenceSurfaceCodec::new()));
    Ok(())
}

/// Reversible codec for complete, certified interference studies.
#[derive(Clone, Copy, Debug, Default)]
pub struct InterferenceSurfaceCodec;

impl InterferenceSurfaceCodec {
    /// Builds the stateless interference surface codec.
    pub const fn new() -> Self {
        Self
    }

    /// Pre-projects a bounded cycle of instantaneous-field Scenes.
    ///
    /// Every frame goes through the domain detector reducer. The total frame
    /// budget is divided before projection, and zero or excessive frame counts
    /// fail before any frame allocation.
    pub fn encode_animation(
        &self,
        cx: &mut Cx,
        value: &Expr,
        caps: &SurfaceCaps,
        frames: usize,
    ) -> Result<Vec<Expr>> {
        let study = intent::decode_study(cx, value)?;
        crate::scene::animation_scenes(cx, &study, caps, frames)
    }
}

impl SurfaceCodec for InterferenceSurfaceCodec {
    fn encode(&self, cx: &mut Cx, value: &Expr, caps: &SurfaceCaps) -> Result<Expr> {
        let study = intent::decode_study(cx, value)?;
        crate::scene::study_scene(cx, &study, caps)
    }

    fn decode(&self, cx: &mut Cx, value: &Expr, submitted: &Expr) -> Result<Draft> {
        let study = intent::decode_study(cx, value)?;
        if let Err(error) = sim_lib_intent::validate_intent(submitted) {
            return Ok(rejected(
                value,
                format!("invalid interference Intent: {error}"),
            ));
        }
        match intent::classify_edit(cx, &study, value, submitted) {
            Ok(edit) => Ok(Draft::clean(value.clone(), intent::encode_edit(cx, &edit)?)),
            Err(error) => Ok(rejected(value, error.to_string())),
        }
    }

    fn commit(&self, cx: &mut Cx, draft: &Draft) -> Result<Operation> {
        if !draft.committable || !draft.diagnostics.is_empty() {
            return Err(Error::HostError(
                "interference draft is not committable".to_owned(),
            ));
        }
        let study = intent::decode_study(cx, &draft.base)?;
        let edit = intent::decode_edit(cx, &study, &draft.proposed)?;
        match edit {
            EditClass::Projection(request) => {
                Ok(Operation::new(intent::project_form(&draft.base, &request))
                    .with_result_shape(registered_shape(cx, projection_shape_symbol())?)
                    .requiring(CapabilityName::new(INTERFERENCE_PROJECT_CAPABILITY)))
            }
            EditClass::Model(request) => Ok(Operation::new(intent::solve_form(cx, &request)?)
                .with_result_shape(registered_shape(cx, study_shape_symbol())?)
                .requiring(CapabilityName::new(INTERFERENCE_SOLVE_CAPABILITY))),
        }
    }
}

fn registered_shape(cx: &mut Cx, symbol: Symbol) -> Result<ShapeRef> {
    install_interference_records(cx)?;
    cx.registry()
        .shape_by_symbol(&symbol)
        .cloned()
        .ok_or(Error::UnknownSymbol { symbol })
}

fn rejected(base: &Expr, message: String) -> Draft {
    Draft::rejected(
        base.clone(),
        Diagnostic::error(message)
            .with_code(Symbol::qualified("interference-surface", "invalid-edit")),
    )
}
