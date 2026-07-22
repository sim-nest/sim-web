//! Glasses-specific consent gates over the shared device consent contract.

use sim_kernel::{CapabilityName, Cx, Error, Expr, Result, Symbol};
use sim_lib_scene::{GlanceCard, GlanceMetric};
use sim_lib_view_device::{
    ConsentReceipt, DeviceSampleStore, EdgeId, Evicted, FrameClock, RetentionReaper, StoreKey,
    StoredSample, require_with_consent,
};
use sim_value::{access, build};

/// Capability required for glasses pose and tracking samples.
pub const CAP_GLASSES_POSE: &str = "glasses/pose";

/// Capability required for glasses camera frames.
pub const CAP_GLASSES_CAMERA: &str = "glasses/camera";

/// Capability required for stable world-anchor observations.
pub const CAP_GLASSES_WORLD_ANCHOR: &str = "glasses/world-anchor";

/// Capability required for glasses hand-ray samples.
pub const CAP_GLASSES_HAND: &str = "glasses/hand";

/// Capability required for glasses microphone capture.
pub const CAP_GLASSES_MIC: &str = "glasses/mic";

/// Capability required for vendor diagnostic reporting.
pub const CAP_GLASSES_VENDOR_REPORT: &str = "glasses/vendor-report";

const XR_NAMESPACE: &str = "xr";
const GLASSES_NAMESPACE: &str = "glasses";

/// Glasses-sensitive capability classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassesCapability {
    /// Pose or spatial tracking samples.
    Pose,
    /// Camera frame references.
    Camera,
    /// Stable world-anchor observations.
    WorldAnchor,
    /// Hand-ray samples.
    Hand,
    /// Raw microphone chunk references.
    Mic,
    /// Vendor diagnostics, off unless explicitly granted.
    VendorReport,
}

impl GlassesCapability {
    /// All glasses-sensitive capabilities.
    pub const ALL: [Self; 6] = [
        Self::Pose,
        Self::Camera,
        Self::WorldAnchor,
        Self::Hand,
        Self::Mic,
        Self::VendorReport,
    ];

    /// Stable kernel capability name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pose => CAP_GLASSES_POSE,
            Self::Camera => CAP_GLASSES_CAMERA,
            Self::WorldAnchor => CAP_GLASSES_WORLD_ANCHOR,
            Self::Hand => CAP_GLASSES_HAND,
            Self::Mic => CAP_GLASSES_MIC,
            Self::VendorReport => CAP_GLASSES_VENDOR_REPORT,
        }
    }

    /// Stable local token after the `glasses/` prefix.
    pub fn local_name(self) -> &'static str {
        match self {
            Self::Pose => "pose",
            Self::Camera => "camera",
            Self::WorldAnchor => "world-anchor",
            Self::Hand => "hand",
            Self::Mic => "mic",
            Self::VendorReport => "vendor-report",
        }
    }

    /// Kernel capability value.
    pub fn capability_name(self) -> CapabilityName {
        CapabilityName::new(self.as_str())
    }

    /// Visible consent grant symbol carried by a [`ConsentReceipt`].
    pub fn grant_symbol(self) -> Symbol {
        Symbol::qualified(GLASSES_NAMESPACE, self.local_name())
    }

    /// Resolves a glasses capability name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.as_str() == name)
    }
}

/// Returns the visible grant symbol for glasses pose samples.
pub fn glasses_pose_grant() -> Symbol {
    GlassesCapability::Pose.grant_symbol()
}

/// Returns the visible grant symbol for glasses camera frames.
pub fn glasses_camera_grant() -> Symbol {
    GlassesCapability::Camera.grant_symbol()
}

/// Returns the visible grant symbol for glasses world-anchor observations.
pub fn glasses_world_anchor_grant() -> Symbol {
    GlassesCapability::WorldAnchor.grant_symbol()
}

/// Returns the visible grant symbol for glasses hand-ray samples.
pub fn glasses_hand_grant() -> Symbol {
    GlassesCapability::Hand.grant_symbol()
}

/// Returns the visible grant symbol for glasses microphone capture.
pub fn glasses_mic_grant() -> Symbol {
    GlassesCapability::Mic.grant_symbol()
}

/// Returns the visible grant symbol for glasses vendor diagnostics.
pub fn glasses_vendor_report_grant() -> Symbol {
    GlassesCapability::VendorReport.grant_symbol()
}

/// Returns the kernel capability name for glasses microphone capture.
pub fn glasses_mic_capability() -> CapabilityName {
    GlassesCapability::Mic.capability_name()
}

/// Classifies an expression into the glasses capability it needs.
pub fn glasses_capability_for_expr(expr: &Expr) -> Result<GlassesCapability> {
    if access::field(expr, "world-anchor").is_some() {
        return Ok(GlassesCapability::WorldAnchor);
    }
    let symbol = access::field_sym(expr, "sample")
        .or_else(|| access::field_sym(expr, "kind"))
        .ok_or_else(|| Error::Eval("missing glasses sample or kind field".to_owned()))?;
    match (symbol.namespace.as_deref(), symbol.name.as_ref()) {
        (Some(XR_NAMESPACE), "pose") => Ok(GlassesCapability::Pose),
        (Some(XR_NAMESPACE), "camera-frame") => Ok(GlassesCapability::Camera),
        (Some(XR_NAMESPACE), "hand") => Ok(GlassesCapability::Hand),
        (Some(XR_NAMESPACE), "mic-chunk") => Ok(GlassesCapability::Mic),
        (Some(GLASSES_NAMESPACE), "world-anchor") => Ok(GlassesCapability::WorldAnchor),
        (Some(GLASSES_NAMESPACE), "vendor-report") => Ok(GlassesCapability::VendorReport),
        _ => Err(Error::HostError(format!(
            "no glasses consent capability for {}",
            symbol.as_qualified_str()
        ))),
    }
}

/// Requires the named glasses capability and a session-bound visible consent receipt.
pub fn require_glasses_consent(
    cx: &Cx,
    capability: GlassesCapability,
    receipt: &ConsentReceipt,
    session: &EdgeId,
) -> Result<()> {
    require_with_consent(cx, capability.as_str(), receipt, session)
}

/// Requires the authority needed to consume one glasses-sensitive expression.
pub fn require_glasses_expr_consent(
    cx: &Cx,
    expr: &Expr,
    receipt: &ConsentReceipt,
    session: &EdgeId,
) -> Result<GlassesCapability> {
    let capability = glasses_capability_for_expr(expr)?;
    require_glasses_consent(cx, capability, receipt, session)?;
    Ok(capability)
}

/// Stores one glasses-sensitive sample under the receipt sequence and modeled clock.
pub fn store_glasses_sample(
    store: &mut DeviceSampleStore,
    capability: GlassesCapability,
    sample_id: impl Into<String>,
    value: Expr,
    receipt: &ConsentReceipt,
    clock: FrameClock,
    content_refs: Vec<StoreKey>,
) -> Result<StoreKey> {
    let sample_id = sample_id.into();
    if sample_id.is_empty() {
        return Err(Error::Eval(
            "glasses sample id must not be empty".to_owned(),
        ));
    }
    let key = StoreKey::new(Symbol::qualified(
        "glasses/store",
        format!("{}-{sample_id}", capability.local_name()),
    ));
    store.insert_sample(StoredSample::new(
        key.clone(),
        receipt.seq,
        clock.tick,
        content_refs,
        stored_glasses_value(capability, value),
    ));
    Ok(key)
}

/// Runs the shared retention reaper for glasses-sensitive samples.
pub fn sweep_glasses_privacy(
    store: &mut DeviceSampleStore,
    receipts: &[ConsentReceipt],
    clock: FrameClock,
) -> Vec<Evicted> {
    RetentionReaper::new().sweep(store, receipts, clock)
}

/// Renders active glasses grants and retention windows for the rich surface.
pub fn active_glasses_consent_badge_cluster(receipts: &[ConsentReceipt]) -> Expr {
    let mut badges = Vec::new();
    for receipt in receipts {
        for grant in glasses_grants(receipt) {
            badges.push(sim_lib_scene::badge(
                "ok",
                &format!("{} {}ms", grant.as_qualified_str(), receipt.retain_ms),
            ));
        }
        if !receipt.redact.is_empty() && receipt_has_glasses_grants(receipt) {
            badges.push(sim_lib_scene::badge(
                "warn",
                &format!("retention {}ms", receipt.retain_ms),
            ));
        }
    }
    sim_lib_scene::badge_cluster(badges)
}

/// Renders active glasses consent as one compact Halo glance card.
pub fn halo_consent_glyph(receipts: &[ConsentReceipt]) -> Expr {
    let grant_count = receipts.iter().flat_map(glasses_grants).count();
    let retain_ms = receipts
        .iter()
        .filter(|receipt| receipt_has_glasses_grants(receipt))
        .map(|receipt| receipt.retain_ms)
        .min()
        .unwrap_or(0);
    GlanceCard::new(
        "Consent",
        Some(GlanceMetric::new(
            "grants",
            format!("{grant_count}/{retain_ms}ms"),
        )),
        None,
        "info",
        1,
    )
    .to_scene()
}

fn stored_glasses_value(capability: GlassesCapability, value: Expr) -> Expr {
    build::map(vec![
        ("kind", build::qsym("glasses", "sensitive-sample")),
        ("capability", Expr::Symbol(capability.grant_symbol())),
        (capability.local_name(), value),
    ])
}

fn glasses_grants(receipt: &ConsentReceipt) -> impl Iterator<Item = &Symbol> {
    receipt
        .grants
        .iter()
        .filter(|grant| grant.namespace.as_deref() == Some(GLASSES_NAMESPACE))
}

fn receipt_has_glasses_grants(receipt: &ConsentReceipt) -> bool {
    glasses_grants(receipt).next().is_some()
}
