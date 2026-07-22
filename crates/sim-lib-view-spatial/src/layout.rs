//! Pose-free spatial layout parsing and Scene builders.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_scene::{Anchor, AnchorSpace, Transform3};
use sim_table_core::{TableOp, TablePath};
use sim_value::{access, build};

/// Namespace for persisted glasses workspace layout records.
pub const WORKSPACE_LAYOUT_NAMESPACE: &str = "workspace";

/// Kind name for persisted glasses workspace layout records.
pub const WORKSPACE_LAYOUT_KIND: &str = "layout";

/// Namespace used for table keys that store glasses workspace layouts.
pub const WORKSPACE_LAYOUT_TABLE_NAMESPACE: &str = "workspace-layout";

const DEFAULT_LAYOUT_KEY: &str = "default";
const DEFAULT_WORLD_ANCHOR: &str = "workspace";

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

    /// Builds the default Viture workspace arc.
    pub fn default_arc() -> Self {
        WorkspaceLayout::default_arc().to_spatial_layout()
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

/// Persisted co-use layout shared by the Viture workspace and Halo glance path.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceLayout {
    panels: Vec<PanelPlacement>,
    glance: GlancePreference,
}

impl WorkspaceLayout {
    /// Builds the default one-panel Viture arc with urgency-first Halo glance.
    pub fn default_arc() -> Self {
        Self {
            panels: vec![
                PanelPlacement::new(
                    Symbol::new("main"),
                    AnchorSpace::World,
                    Transform3::new([0.0, 1.2, -1.6], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]),
                )
                .with_world_anchor(Symbol::new(DEFAULT_WORLD_ANCHOR)),
            ],
            glance: GlancePreference::default(),
        }
    }

    /// Builds a workspace layout from explicit placements and glance preference.
    pub fn new(panels: Vec<PanelPlacement>, glance: GlancePreference) -> Result<Self> {
        if panels.is_empty() {
            return Err(Error::HostError(
                "workspace/layout must contain at least one panel".to_owned(),
            ));
        }
        Ok(Self { panels, glance })
    }

    /// Returns Viture panel placements in render order.
    pub fn panels(&self) -> &[PanelPlacement] {
        &self.panels
    }

    /// Returns the Halo glance preference stored with the workspace layout.
    pub fn glance(&self) -> &GlancePreference {
        &self.glance
    }

    /// Encodes the layout as a portable SIM `Expr`.
    pub fn to_expr(&self) -> Expr {
        build::map(vec![
            (
                "kind",
                Expr::Symbol(Symbol::qualified(
                    WORKSPACE_LAYOUT_NAMESPACE,
                    WORKSPACE_LAYOUT_KIND,
                )),
            ),
            (
                "panels",
                build::list(self.panels.iter().map(PanelPlacement::to_expr).collect()),
            ),
            ("glance", self.glance.to_expr()),
        ])
    }

    /// Decodes a portable SIM `Expr` into a workspace layout.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        reject_runtime_fields(expr)?;
        expect_kind(expr, WORKSPACE_LAYOUT_KIND, "workspace/layout")?;
        let panels_expr = access::required(expr, "panels", "workspace/layout")?;
        let panels = match panels_expr {
            Expr::List(items) | Expr::Vector(items) => items
                .iter()
                .map(PanelPlacement::from_expr)
                .collect::<Result<Vec<_>>>()?,
            _ => {
                return Err(Error::HostError(
                    "workspace/layout panels field must be a list".to_owned(),
                ));
            }
        };
        let glance = access::field(expr, "glance")
            .map(GlancePreference::from_expr)
            .transpose()?
            .unwrap_or_default();
        Self::new(panels, glance)
    }

    fn to_spatial_layout(&self) -> SpatialLayout {
        SpatialLayout {
            panels: self
                .panels
                .iter()
                .map(PanelPlacement::to_panel_layout)
                .collect(),
        }
    }
}

/// A persisted Viture panel placement.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelPlacement {
    /// Stable panel id.
    pub panel_id: Symbol,
    /// Pose-free coordinate space for the panel.
    pub space: AnchorSpace,
    /// Static transform applied before device pose.
    pub transform: Transform3,
    /// Optional stable world anchor id.
    pub world_anchor: Option<Symbol>,
}

impl PanelPlacement {
    /// Builds a placement without a world anchor.
    pub fn new(panel_id: Symbol, space: AnchorSpace, transform: Transform3) -> Self {
        Self {
            panel_id,
            space,
            transform,
            world_anchor: None,
        }
    }

    /// Attaches a stable world anchor id.
    pub fn with_world_anchor(mut self, world_anchor: Symbol) -> Self {
        self.world_anchor = Some(world_anchor);
        self
    }

    /// Encodes the placement as portable workspace layout data.
    pub fn to_expr(&self) -> Expr {
        let mut fields = vec![
            (
                "kind",
                Expr::Symbol(Symbol::qualified(
                    WORKSPACE_LAYOUT_NAMESPACE,
                    "panel-placement",
                )),
            ),
            ("panel-id", Expr::Symbol(self.panel_id.clone())),
            ("space", self.space.to_expr()),
            ("transform", self.transform.to_expr()),
        ];
        if let Some(anchor) = &self.world_anchor {
            fields.push(("world-anchor", Expr::Symbol(anchor.clone())));
        }
        build::map(fields)
    }

    /// Decodes one persisted panel placement.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        reject_runtime_fields(expr)?;
        expect_kind(expr, "panel-placement", "workspace/panel-placement")?;
        let panel_id = access::required_sym(expr, "panel-id", "workspace/panel-placement")?;
        let space = AnchorSpace::from_expr(access::required(
            expr,
            "space",
            "workspace/panel-placement",
        )?)?;
        let transform = Transform3::from_expr(access::required(
            expr,
            "transform",
            "workspace/panel-placement",
        )?)?;
        Ok(Self {
            panel_id,
            space,
            transform,
            world_anchor: access::field_sym(expr, "world-anchor"),
        })
    }

    fn to_panel_layout(&self) -> PanelLayout {
        let target = self
            .world_anchor
            .as_ref()
            .map(Symbol::as_qualified_str)
            .unwrap_or_else(|| DEFAULT_WORLD_ANCHOR.to_owned());
        PanelLayout {
            id: self.panel_id.as_qualified_str(),
            anchor: Anchor::new(self.space, target),
            transform: self.transform.clone(),
        }
    }
}

/// Halo glance selection policy stored with a workspace layout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GlancePreference {
    /// Keep the shared glance reducer's urgency-first selection.
    #[default]
    UrgencyFirst,
    /// Prefer a particular item class when the host reducer supports it.
    ItemClass(Symbol),
}

impl GlancePreference {
    /// Builds a preference for an item class.
    pub fn item_class(class: Symbol) -> Self {
        Self::ItemClass(class)
    }

    /// Returns the preferred item class, if one is set.
    pub fn preferred_item_class(&self) -> Option<&Symbol> {
        match self {
            Self::UrgencyFirst => None,
            Self::ItemClass(class) => Some(class),
        }
    }

    /// Encodes the preference as portable workspace layout data.
    pub fn to_expr(&self) -> Expr {
        let mut fields = vec![
            (
                "kind",
                Expr::Symbol(Symbol::qualified(
                    WORKSPACE_LAYOUT_NAMESPACE,
                    "glance-preference",
                )),
            ),
            (
                "mode",
                build::sym(match self {
                    Self::UrgencyFirst => "urgency-first",
                    Self::ItemClass(_) => "item-class",
                }),
            ),
        ];
        if let Self::ItemClass(class) = self {
            fields.push(("item-class", Expr::Symbol(class.clone())));
        }
        build::map(fields)
    }

    /// Decodes a Halo glance preference.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        reject_runtime_fields(expr)?;
        expect_kind(expr, "glance-preference", "workspace/glance-preference")?;
        let mode = access::required_sym(expr, "mode", "workspace/glance-preference")?;
        if mode.namespace.is_some() {
            return Err(Error::HostError(
                "workspace/glance-preference mode must be unqualified".to_owned(),
            ));
        }
        match mode.name.as_ref() {
            "urgency-first" => Ok(Self::UrgencyFirst),
            "item-class" => Ok(Self::ItemClass(access::required_sym(
                expr,
                "item-class",
                "workspace/glance-preference",
            )?)),
            other => Err(Error::HostError(format!(
                "unknown workspace/glance-preference mode {other}"
            ))),
        }
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
    access::field(value, "workspace-layout")
        .or_else(|| access::field(value, "spatial-workspace-layout"))
        .or_else(|| access::field(value, "spatial-layout"))
        .or_else(|| access::field(value, "layout"))
}

/// Wraps a flat Scene in a pose-free `scene/spatial` panel layout.
pub fn arrange_spatial_panels(scene: Expr, layout: Option<&Expr>) -> Result<Expr> {
    let layout = layout
        .map(spatial_layout_from_expr)
        .transpose()?
        .unwrap_or_else(SpatialLayout::default_arc);
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

/// Builds the table key used for persisted workspace layouts.
pub fn layout_table_key(path: &TablePath) -> Symbol {
    let key = if path.segments().is_empty() {
        DEFAULT_LAYOUT_KEY.to_owned()
    } else {
        path.segments().join(".")
    };
    Symbol::qualified(WORKSPACE_LAYOUT_TABLE_NAMESPACE, key)
}

/// Builds a `table/set` operation storing `layout` at `path`.
pub fn layout_save_op(path: &TablePath, layout: &WorkspaceLayout) -> TableOp {
    TableOp::Set(layout_table_key(path), layout.to_expr())
}

/// Builds a `table/get` operation loading the layout stored at `path`.
pub fn layout_load_op(path: &TablePath) -> TableOp {
    TableOp::Get(layout_table_key(path))
}

fn spatial_layout_from_expr(expr: &Expr) -> Result<SpatialLayout> {
    if access::field(expr, "kind").is_some() {
        WorkspaceLayout::from_expr(expr).map(|layout| layout.to_spatial_layout())
    } else {
        SpatialLayout::from_expr(expr)
    }
}

fn expect_kind(expr: &Expr, name: &str, context: &str) -> Result<()> {
    let kind = access::required_sym(expr, "kind", context)?;
    if kind.namespace.as_deref() == Some(WORKSPACE_LAYOUT_NAMESPACE) && kind.name.as_ref() == name {
        Ok(())
    } else {
        Err(Error::HostError(format!(
            "{context} kind must be {WORKSPACE_LAYOUT_NAMESPACE}/{name}"
        )))
    }
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
