//! Attention ranking for spatial glasses projection.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_view_device::{DeviceProfile, GlassesClass, glasses_class};
use sim_value::{access, build};

const DEFAULT_GAZE: [f64; 3] = [0.0, 0.0, -1.0];

/// Content-rate attention budget for spatial glasses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttentionBudget {
    /// Maximum gaze angle for foveal detail.
    pub foveal_deg: f64,
    /// Maximum gaze angle for peripheral detail.
    pub peripheral_deg: f64,
    /// Maximum non-critical panels lit at once.
    pub max_lit_panels: usize,
    /// Whether a camera stream is live and should raise the privacy shade.
    pub camera_live: bool,
}

impl AttentionBudget {
    /// Builds a budget with stable spatial-glasses defaults.
    pub fn new(max_lit_panels: usize) -> Self {
        Self {
            foveal_deg: 12.0,
            peripheral_deg: 55.0,
            max_lit_panels,
            camera_live: false,
        }
    }

    /// Returns the default Viture-style spatial budget.
    pub fn spatial_default() -> Self {
        Self::new(4)
    }

    /// Builds a content-rate budget from a device profile.
    pub fn for_profile(profile: &DeviceProfile) -> Self {
        Self {
            camera_live: has_symbol(&profile.input, "camera")
                || has_symbol(&profile.streams, "camera"),
            ..Self::spatial_default()
        }
    }

    /// Sets whether the privacy shade is raised.
    pub fn with_camera_live(mut self, camera_live: bool) -> Self {
        self.camera_live = camera_live;
        self
    }
}

/// Ranks a glasses scene according to the selected glasses path.
///
/// Stereo 6DoF scenes are spatially ranked. Mono HUD and display-only scenes are
/// returned unchanged because their budget is owned by the DEVICE_3 glance or
/// mirror paths.
pub fn rank_glasses(
    scene: &Expr,
    class: GlassesClass,
    gaze: [f64; 3],
    budget: &AttentionBudget,
) -> Result<Expr> {
    match class {
        GlassesClass::Stereo6Dof => rank_spatial(scene, gaze, budget),
        GlassesClass::MonoHud | GlassesClass::DisplayOnly => Ok(scene.clone()),
    }
}

/// Ranks a scene for an already-derived profile using the default forward gaze.
pub fn rank_for_profile(scene: &Expr, profile: &DeviceProfile) -> Result<Expr> {
    match glasses_class(profile) {
        Some(class) => rank_glasses(
            scene,
            class,
            DEFAULT_GAZE,
            &AttentionBudget::for_profile(profile),
        ),
        None => Ok(scene.clone()),
    }
}

/// Applies foveal, peripheral, budget, and privacy-shade metadata to a spatial scene.
pub fn rank_spatial(scene: &Expr, gaze: [f64; 3], budget: &AttentionBudget) -> Result<Expr> {
    expect_scene_kind(scene, "spatial")?;
    let children = spatial_children(scene)?;
    let seeds = children
        .iter()
        .enumerate()
        .filter(|(_, child)| matches!(scene_kind(child).as_deref(), Some("panel")))
        .map(|(index, child)| panel_seed(index, child, gaze, budget))
        .collect::<Result<Vec<_>>>()?;
    let lit = lit_panels(&seeds, budget.max_lit_panels);
    let ranked_children = children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            seeds
                .iter()
                .find(|seed| seed.index == index)
                .map(|seed| annotate_panel(child, seed, lit[index]))
                .unwrap_or_else(|| child.clone())
        })
        .collect();
    let mut ranked = access::set(scene, "children", build::list(ranked_children));
    ranked = access::set(&ranked, "privacy-shade", Expr::Bool(budget.camera_live));
    if budget.camera_live {
        ranked = access::set(&ranked, "privacy-reason", build::sym("camera-live"));
    }
    Ok(ranked)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttentionDetail {
    Foveal,
    Peripheral,
    Hidden,
}

impl AttentionDetail {
    fn token(self) -> &'static str {
        match self {
            Self::Foveal => "foveal",
            Self::Peripheral => "peripheral",
            Self::Hidden => "hidden",
        }
    }

    fn order(self) -> u8 {
        match self {
            Self::Foveal => 0,
            Self::Peripheral => 1,
            Self::Hidden => 2,
        }
    }
}

#[derive(Clone, Debug)]
struct PanelSeed {
    index: usize,
    angle_deg: f64,
    detail: AttentionDetail,
    pinned: bool,
}

impl PanelSeed {
    fn eligible(&self) -> bool {
        self.pinned || self.detail != AttentionDetail::Hidden
    }
}

fn panel_seed(
    index: usize,
    panel: &Expr,
    gaze: [f64; 3],
    budget: &AttentionBudget,
) -> Result<PanelSeed> {
    let angle_deg = angle_deg(gaze, panel_direction(panel)?);
    let pinned = has_warrant_or_error(panel);
    let mut detail = if angle_deg <= budget.foveal_deg {
        AttentionDetail::Foveal
    } else if angle_deg <= budget.peripheral_deg {
        AttentionDetail::Peripheral
    } else {
        AttentionDetail::Hidden
    };
    if pinned && detail == AttentionDetail::Hidden {
        detail = AttentionDetail::Peripheral;
    }
    Ok(PanelSeed {
        index,
        angle_deg,
        detail,
        pinned,
    })
}

fn lit_panels(seeds: &[PanelSeed], max_lit_panels: usize) -> Vec<bool> {
    let mut lit = vec![false; seeds.iter().map(|seed| seed.index).max().unwrap_or(0) + 1];
    for seed in seeds {
        if seed.pinned {
            lit[seed.index] = true;
        }
    }
    let mut normal = seeds
        .iter()
        .filter(|seed| !seed.pinned && seed.eligible())
        .collect::<Vec<_>>();
    normal.sort_by(|a, b| {
        a.detail
            .order()
            .cmp(&b.detail.order())
            .then_with(|| a.angle_deg.total_cmp(&b.angle_deg))
            .then_with(|| a.index.cmp(&b.index))
    });
    for seed in normal.into_iter().take(max_lit_panels) {
        lit[seed.index] = true;
    }
    lit
}

fn annotate_panel(panel: &Expr, seed: &PanelSeed, lit: bool) -> Expr {
    let detail = if lit {
        seed.detail
    } else {
        AttentionDetail::Hidden
    };
    let reason = if seed.pinned {
        "pinned"
    } else if !seed.eligible() || lit {
        "gaze"
    } else {
        "budget"
    };
    let rank = if lit { 1000.0 - seed.angle_deg } else { 0.0 };
    let mut out = access::set(panel, "attention-detail", build::sym(detail.token()));
    out = access::set(&out, "attention-angle-deg", build::float(seed.angle_deg));
    out = access::set(&out, "attention-rank", build::float(rank));
    out = access::set(&out, "attention-lit", Expr::Bool(lit));
    out = access::set(&out, "attention-pinned", Expr::Bool(seed.pinned));
    access::set(&out, "attention-reason", build::sym(reason))
}

fn panel_direction(panel: &Expr) -> Result<[f64; 3]> {
    let Some(transform) = access::field(panel, "transform") else {
        return Ok(DEFAULT_GAZE);
    };
    let Some(value) = access::field(transform, "translate-m") else {
        return Ok(DEFAULT_GAZE);
    };
    let Expr::Vector(items) = value else {
        return Err(Error::HostError(
            "scene/panel transform translate-m must be a vector".to_owned(),
        ));
    };
    if items.len() != 3 {
        return Err(Error::HostError(
            "scene/panel transform translate-m must contain 3 numbers".to_owned(),
        ));
    }
    let mut out = [0.0; 3];
    for (index, item) in items.iter().enumerate() {
        let value = access::as_f64(item).ok_or_else(|| {
            Error::HostError(format!(
                "scene/panel transform translate-m[{index}] must be numeric"
            ))
        })?;
        if !value.is_finite() {
            return Err(Error::HostError(format!(
                "scene/panel transform translate-m[{index}] must be finite"
            )));
        }
        out[index] = value;
    }
    if magnitude(out) == 0.0 {
        Ok(DEFAULT_GAZE)
    } else {
        Ok(out)
    }
}

fn angle_deg(gaze: [f64; 3], direction: [f64; 3]) -> f64 {
    let gaze = normalize_or_default(gaze);
    let direction = normalize_or_default(direction);
    dot(gaze, direction).clamp(-1.0, 1.0).acos().to_degrees()
}

fn normalize_or_default(vector: [f64; 3]) -> [f64; 3] {
    let magnitude = magnitude(vector);
    if magnitude == 0.0 || !magnitude.is_finite() {
        DEFAULT_GAZE
    } else {
        [
            vector[0] / magnitude,
            vector[1] / magnitude,
            vector[2] / magnitude,
        ]
    }
}

fn magnitude(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn has_warrant_or_error(expr: &Expr) -> bool {
    field_has_critical_symbol(expr, "status")
        || field_has_critical_symbol(expr, "urgency")
        || access::field_bool(expr, "warrant").unwrap_or(false)
        || scene_kind(expr).as_deref() == Some("warrant")
        || children(expr).any(has_warrant_or_error)
}

fn field_has_critical_symbol(expr: &Expr, name: &str) -> bool {
    access::field_sym(expr, name).is_some_and(|symbol| {
        matches!(
            symbol.name.as_ref(),
            "error" | "critical" | "warrant" | "warn"
        )
    })
}

fn children(expr: &Expr) -> impl Iterator<Item = &Expr> {
    match expr {
        Expr::Map(entries) => entries.iter().map(|(_, value)| value).collect::<Vec<_>>(),
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => items.iter().collect(),
        _ => Vec::new(),
    }
    .into_iter()
}

fn spatial_children(scene: &Expr) -> Result<&[Expr]> {
    match access::required(scene, "children", "scene/spatial")? {
        Expr::List(children) => Ok(children),
        _ => Err(Error::HostError(
            "scene/spatial children must be a list".to_owned(),
        )),
    }
}

fn expect_scene_kind(scene: &Expr, expected: &str) -> Result<()> {
    match scene_kind(scene).as_deref() {
        Some(kind) if kind == expected => Ok(()),
        _ => Err(Error::HostError(format!("expected scene/{expected}"))),
    }
}

fn scene_kind(expr: &Expr) -> Option<String> {
    let kind = sim_lib_scene::node_kind(expr)?;
    (kind.namespace.as_deref() == Some(sim_lib_scene::SCENE_NAMESPACE))
        .then(|| kind.name.to_string())
}

fn has_symbol(symbols: &[Symbol], name: &str) -> bool {
    symbols
        .iter()
        .any(|symbol| symbol.namespace.is_none() && symbol.name.as_ref() == name)
}
