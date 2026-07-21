//! Pose-free spatial layout parsing and Scene builders.

use sim_kernel::{Error, Expr, Result};
use sim_lib_scene::{Anchor, AnchorSpace, Transform3};
use sim_value::access;

/// A spatial layout made of anchored panels.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialLayout {
    panels: Vec<PanelLayout>,
}

impl SpatialLayout {
    /// Builds the default single-panel layout.
    pub fn single_panel() -> Self {
        Self {
            panels: vec![PanelLayout::default()],
        }
    }

    /// Parses a layout expression.
    ///
    /// A layout is a map with an optional `panels` list. Each panel may carry
    /// `id`, `anchor`, and `transform` fields. Missing fields use stable
    /// defaults so a value can be encoded spatially without layout metadata.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        reject_runtime_fields(expr)?;
        let Some(Expr::List(items) | Expr::Vector(items)) = access::field(expr, "panels") else {
            return Ok(Self::single_panel());
        };
        if items.is_empty() {
            return Ok(Self::single_panel());
        }
        let panels = items
            .iter()
            .enumerate()
            .map(|(index, item)| PanelLayout::from_expr(index, item))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { panels })
    }

    /// Returns the panel layouts in order.
    pub fn panels(&self) -> &[PanelLayout] {
        &self.panels
    }
}

/// A single anchored panel in a spatial layout.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelLayout {
    /// Stable panel id.
    pub id: String,
    /// Pose-free anchor for the panel.
    pub anchor: Anchor,
    /// Static transform applied at render time.
    pub transform: Transform3,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            id: "panel-0".to_owned(),
            anchor: Anchor::new(AnchorSpace::World, "workspace"),
            transform: Transform3::identity(),
        }
    }
}

impl PanelLayout {
    fn from_expr(index: usize, expr: &Expr) -> Result<Self> {
        reject_runtime_fields(expr)?;
        let default = Self {
            id: format!("panel-{index}"),
            ..Self::default()
        };
        Ok(Self {
            id: access::field_str(expr, "id")
                .map(str::to_owned)
                .unwrap_or(default.id),
            anchor: access::field(expr, "anchor")
                .map(Anchor::from_expr)
                .transpose()?
                .unwrap_or(default.anchor),
            transform: access::field(expr, "transform")
                .map(Transform3::from_expr)
                .transpose()?
                .unwrap_or(default.transform),
        })
    }
}

/// Returns optional spatial layout metadata embedded in a value.
pub fn layout_expr(value: &Expr) -> Option<&Expr> {
    access::field(value, "spatial-layout").or_else(|| access::field(value, "layout"))
}

/// Wraps a flat Scene in a pose-free `scene/spatial` panel layout.
pub fn arrange_spatial_panels(scene: Expr, layout: Option<&Expr>) -> Result<Expr> {
    let layout = layout
        .map(SpatialLayout::from_expr)
        .transpose()?
        .unwrap_or_else(SpatialLayout::single_panel);
    let panels = layout
        .panels()
        .iter()
        .map(|panel| {
            sim_lib_scene::panel(
                panel.id.clone(),
                scene.clone(),
                panel.anchor.clone(),
                panel.transform.clone(),
            )
        })
        .collect();
    Ok(sim_lib_scene::spatial(panels))
}

fn reject_runtime_fields(expr: &Expr) -> Result<()> {
    match expr {
        Expr::Map(entries) => {
            for (key, value) in entries {
                if is_runtime_key(key) {
                    return Err(Error::HostError(
                        "spatial layout must not carry runtime tracking fields".to_owned(),
                    ));
                }
                reject_runtime_fields(value)?;
            }
        }
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => {
            for item in items {
                reject_runtime_fields(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_runtime_key(key: &Expr) -> bool {
    matches!(
        key,
        Expr::Symbol(symbol)
            if symbol.namespace.is_none() && matches!(symbol.name.as_ref(), "pose" | "tick")
    )
}
