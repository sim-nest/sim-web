//! World-anchor resolution for Viture spatial panels.

use std::collections::BTreeMap;

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_scene::{AnchorSpace, Transform3};

use crate::PanelPlacement;

/// Namespace shared with XR tracking-status sample symbols.
pub const XR_TRACKING_STATUS_NAMESPACE: &str = "stream/xr-tracking";

/// Namespace for world-anchor fallback reason symbols.
pub const WORLD_ANCHOR_REASON_NAMESPACE: &str = "world-anchor";

/// VIO stability state used by the spatial world-anchor resolver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VioTrackingStatus {
    /// Six degree-of-freedom tracking is stable enough for world-locked panels.
    Stable6Dof,
    /// Tracking is present but not stable enough to keep panels world locked.
    Limited,
    /// Tracking is unavailable.
    Lost,
}

impl VioTrackingStatus {
    /// Returns whether this status can keep world anchors locked.
    pub fn is_stable(self) -> bool {
        matches!(self, Self::Stable6Dof)
    }

    /// Encodes the status as the shared XR tracking symbol.
    pub fn to_symbol(self) -> Symbol {
        match self {
            Self::Stable6Dof => Symbol::qualified(XR_TRACKING_STATUS_NAMESPACE, "tracked"),
            Self::Limited => Symbol::qualified(XR_TRACKING_STATUS_NAMESPACE, "limited"),
            Self::Lost => Symbol::qualified(XR_TRACKING_STATUS_NAMESPACE, "lost"),
        }
    }

    /// Decodes the shared XR tracking symbol into resolver status.
    pub fn from_symbol(symbol: &Symbol) -> Result<Self> {
        match (symbol.namespace.as_deref(), symbol.name.as_ref()) {
            (Some(XR_TRACKING_STATUS_NAMESPACE), "tracked") => Ok(Self::Stable6Dof),
            (Some(XR_TRACKING_STATUS_NAMESPACE), "limited") => Ok(Self::Limited),
            (Some(XR_TRACKING_STATUS_NAMESPACE), "lost") => Ok(Self::Lost),
            _ => Err(Error::HostError(format!(
                "unknown Viture VIO tracking status {symbol}"
            ))),
        }
    }

    /// Encodes the status as portable expression data.
    pub fn to_expr(self) -> Expr {
        Expr::Symbol(self.to_symbol())
    }

    /// Decodes portable expression data into resolver status.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let Expr::Symbol(symbol) = expr else {
            return Err(Error::HostError(
                "Viture VIO tracking status must be a symbol".to_owned(),
            ));
        };
        Self::from_symbol(symbol)
    }
}

/// One observed world anchor or plane that can support a panel placement.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldAnchorObservation {
    /// Stable anchor id observed by VIO.
    pub anchor: Symbol,
    /// World-space transform for the observed anchor.
    pub transform: Transform3,
}

impl WorldAnchorObservation {
    /// Builds an observed world anchor.
    pub fn new(anchor: Symbol, transform: Transform3) -> Self {
        Self { anchor, transform }
    }
}

/// Result of resolving a panel placement against world observations and VIO state.
#[derive(Clone, Debug, PartialEq)]
pub enum AnchorResolution {
    /// Placement remains pinned to a stable world anchor.
    World {
        /// Stable anchor id used for the resolution.
        anchor: Symbol,
        /// Resolved world-space transform.
        transform: Transform3,
    },
    /// Placement is rendered head locked until world tracking is safe again.
    HeadLocked {
        /// Stable anchor id requested by the placement.
        anchor: Symbol,
        /// Head-relative transform used while degraded.
        transform: Transform3,
        /// Reason for the degradation.
        reason: Symbol,
    },
}

impl AnchorResolution {
    /// Returns the anchor id involved in this resolution.
    pub fn anchor(&self) -> &Symbol {
        match self {
            Self::World { anchor, .. } | Self::HeadLocked { anchor, .. } => anchor,
        }
    }

    /// Returns the transform to render.
    pub fn transform(&self) -> &Transform3 {
        match self {
            Self::World { transform, .. } | Self::HeadLocked { transform, .. } => transform,
        }
    }

    /// Returns the resolved coordinate space.
    pub fn anchor_space(&self) -> AnchorSpace {
        match self {
            Self::World { .. } => AnchorSpace::World,
            Self::HeadLocked { .. } => AnchorSpace::Head,
        }
    }

    /// Returns the fallback reason when the panel is head locked.
    pub fn reason(&self) -> Option<&Symbol> {
        match self {
            Self::World { .. } => None,
            Self::HeadLocked { reason, .. } => Some(reason),
        }
    }
}

/// Resolver for observed world anchors.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldAnchorResolver {
    anchors: BTreeMap<Symbol, Transform3>,
}

impl WorldAnchorResolver {
    /// Builds a resolver from observed anchors.
    pub fn new(observed: impl IntoIterator<Item = WorldAnchorObservation>) -> Self {
        let anchors = observed
            .into_iter()
            .map(|item| (item.anchor, item.transform))
            .collect();
        Self { anchors }
    }

    /// Records or replaces one observed anchor transform.
    pub fn observe(&mut self, observation: WorldAnchorObservation) {
        self.anchors
            .insert(observation.anchor, observation.transform);
    }

    /// Returns the observed transform for `anchor`.
    pub fn observed_transform(&self, anchor: &Symbol) -> Option<&Transform3> {
        self.anchors.get(anchor)
    }

    /// Resolves one panel placement for the current VIO status.
    pub fn resolve(
        &self,
        placement: &PanelPlacement,
        status: VioTrackingStatus,
    ) -> AnchorResolution {
        let anchor = placement_anchor(placement);
        if placement.space != AnchorSpace::World {
            return head_locked(placement, anchor, reason("non-world-anchor"));
        }
        if !status.is_stable() {
            return head_locked(placement, anchor, tracking_reason(status));
        }
        let Some(observed) = self.anchors.get(&anchor) else {
            return head_locked(placement, anchor, reason("missing-world-anchor"));
        };
        AnchorResolution::World {
            anchor,
            transform: compose_transforms(observed, &placement.transform),
        }
    }
}

/// Resolves one panel placement against observed anchors and VIO status.
pub fn resolve_world_anchor(
    placement: &PanelPlacement,
    status: VioTrackingStatus,
    resolver: &WorldAnchorResolver,
) -> AnchorResolution {
    resolver.resolve(placement, status)
}

fn placement_anchor(placement: &PanelPlacement) -> Symbol {
    placement
        .world_anchor
        .clone()
        .unwrap_or_else(|| placement.panel_id.clone())
}

fn head_locked(placement: &PanelPlacement, anchor: Symbol, reason: Symbol) -> AnchorResolution {
    AnchorResolution::HeadLocked {
        anchor,
        transform: placement.transform.clone(),
        reason,
    }
}

fn tracking_reason(status: VioTrackingStatus) -> Symbol {
    match status {
        VioTrackingStatus::Stable6Dof => reason("stable"),
        VioTrackingStatus::Limited => reason("unstable-vio"),
        VioTrackingStatus::Lost => reason("lost-vio"),
    }
}

fn reason(name: &'static str) -> Symbol {
    Symbol::qualified(WORLD_ANCHOR_REASON_NAMESPACE, name)
}

fn compose_transforms(anchor: &Transform3, local: &Transform3) -> Transform3 {
    let translated = rotate_vector(
        normalize_quat(anchor.rotate_xyzw),
        [
            local.translate_m[0] * anchor.scale[0],
            local.translate_m[1] * anchor.scale[1],
            local.translate_m[2] * anchor.scale[2],
        ],
    );
    Transform3::new(
        [
            anchor.translate_m[0] + translated[0],
            anchor.translate_m[1] + translated[1],
            anchor.translate_m[2] + translated[2],
        ],
        normalize_quat(quat_mul(anchor.rotate_xyzw, local.rotate_xyzw)),
        [
            anchor.scale[0] * local.scale[0],
            anchor.scale[1] * local.scale[1],
            anchor.scale[2] * local.scale[2],
        ],
    )
}

fn quat_mul(left: [f64; 4], right: [f64; 4]) -> [f64; 4] {
    let [x1, y1, z1, w1] = normalize_quat(left);
    let [x2, y2, z2, w2] = normalize_quat(right);
    [
        w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
        w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
        w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
        w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2,
    ]
}

fn normalize_quat(quat: [f64; 4]) -> [f64; 4] {
    let len =
        (quat[0] * quat[0] + quat[1] * quat[1] + quat[2] * quat[2] + quat[3] * quat[3]).sqrt();
    if len == 0.0 {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [quat[0] / len, quat[1] / len, quat[2] / len, quat[3] / len]
    }
}

fn rotate_vector(quat: [f64; 4], vector: [f64; 3]) -> [f64; 3] {
    let qv = [quat[0], quat[1], quat[2]];
    let uv = cross(qv, vector);
    let uuv = cross(qv, uv);
    [
        vector[0] + 2.0 * (quat[3] * uv[0] + uuv[0]),
        vector[1] + 2.0 * (quat[3] * uv[1] + uuv[1]),
        vector[2] + 2.0 * (quat[3] * uv[2] + uuv[2]),
    ]
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}
