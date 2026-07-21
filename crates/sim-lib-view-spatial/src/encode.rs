//! Surface codec implementation for spatial-capable glasses.

use std::borrow::Cow;
use std::sync::Arc;

use sim_kernel::{Cx, Error, Expr, Result, Symbol};
use sim_lib_view::{
    Draft, Operation, PairCodec, SurfaceCaps, SurfaceCodec, UniversalEditor, UniversalView, View,
    codec::reduce_for_caps,
};
use sim_lib_view_device::{DeviceSurfaceCapsExt, GlassesClass, glasses_class};

use crate::glance_map::halo_glance_scene;
use crate::layout::{arrange_spatial_panels, layout_expr};
use crate::rank::rank_for_profile;

/// The id under which the spatial glasses surface codec is registered.
pub const SPATIAL_SURFACE_CODEC_ID: &str = "surface:spatial";

/// Symbol form of [`SPATIAL_SURFACE_CODEC_ID`].
pub fn surface_spatial_codec_symbol() -> Symbol {
    Symbol::new(SPATIAL_SURFACE_CODEC_ID)
}

/// Capability-aware codec for glasses surfaces.
pub struct SpatialSurfaceCodec {
    editor: PairCodec,
}

impl Default for SpatialSurfaceCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl SpatialSurfaceCodec {
    /// Builds a spatial surface codec backed by the universal editor.
    pub fn new() -> Self {
        Self {
            editor: PairCodec::new(
                Arc::new(UniversalView),
                Arc::new(UniversalEditor::writable()),
            ),
        }
    }

    fn source_scene(&self, cx: &mut Cx, value: &Expr) -> Result<Expr> {
        let view_value = strip_layout_metadata(value);
        let scene = UniversalView.encode(cx, view_value.as_ref())?;
        validate("universal view produced invalid Scene", &scene)?;
        Ok(scene)
    }
}

impl SurfaceCodec for SpatialSurfaceCodec {
    fn encode(&self, cx: &mut Cx, value: &Expr, caps: &SurfaceCaps) -> Result<Expr> {
        let scene = self.source_scene(cx, value)?;
        let profile = caps.device_profile();
        let encoded = match glasses_class(&profile) {
            Some(GlassesClass::Stereo6Dof) => rank_for_profile(
                &arrange_spatial_panels(scene, layout_expr(value))?,
                &profile,
            )?,
            Some(GlassesClass::MonoHud) => halo_glance_scene(&scene, &profile)?,
            Some(GlassesClass::DisplayOnly) | None => reduce_for_caps(&scene, caps),
        };
        validate("spatial surface produced invalid Scene", &encoded)?;
        Ok(encoded)
    }

    fn decode(&self, cx: &mut Cx, value: &Expr, intent: &Expr) -> Result<Draft> {
        self.editor.decode(cx, value, intent)
    }

    fn commit(&self, cx: &mut Cx, draft: &Draft) -> Result<Operation> {
        self.editor.commit(cx, draft)
    }
}

fn validate(context: &str, scene: &Expr) -> Result<()> {
    sim_lib_scene::validate_scene(scene)
        .map_err(|err| Error::HostError(format!("{context}: {err}")))
}

fn strip_layout_metadata(value: &Expr) -> Cow<'_, Expr> {
    let Expr::Map(entries) = value else {
        return Cow::Borrowed(value);
    };
    let filtered = entries
        .iter()
        .filter(|(key, _)| !is_layout_metadata_key(key))
        .cloned()
        .collect::<Vec<_>>();
    if filtered.len() == entries.len() {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(Expr::Map(filtered))
    }
}

fn is_layout_metadata_key(key: &Expr) -> bool {
    let name = match key {
        Expr::Symbol(symbol) if symbol.namespace.is_none() => symbol.name.as_ref(),
        Expr::String(text) => text.as_str(),
        _ => return false,
    };
    matches!(
        name,
        "workspace-layout" | "spatial-workspace-layout" | "spatial-layout" | "layout"
    )
}
