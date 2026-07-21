//! Glasses projections for pending BRIDGE/FORGE warrant gates.
//!
//! A packet with a warrant is a human gate. This module projects that gate into
//! the glasses-specific Scene shapes that the existing device paths already
//! understand: a pinned spatial panel for Viture and a DEVICE_3 glance card for
//! Halo. The decision itself remains an ordinary `intent/approve` or
//! `intent/reject` value.

use sim_codec_bridge::{BridgePacket, BridgeWarrant, content_id_string};
use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_scene::{Anchor, AnchorSpace, Transform3};
use sim_lib_view::SurfaceCaps;
use sim_lib_view_device::{DeviceProfile, DeviceSurfaceCapsExt, GlanceReducer, GlassesClass};
use sim_value::{access, build};

/// Namespace used by the standard BRIDGE packet-review mission.
pub const BRIDGE_WARRANT_REVIEW_MISSION_NAMESPACE: &str = "bridge";

/// Name used by the standard BRIDGE packet-review mission.
pub const BRIDGE_WARRANT_REVIEW_MISSION_NAME: &str = "packet-review";

/// Stable id for the Viture spatial warrant-review panel.
pub const VITURE_WARRANT_REVIEW_PANEL_ID: &str = "bridge-warrant-review";

const HALO_GLYPH: &str = "OK/X";
const REVIEW_TITLE: &str = "BRIDGE/FORGE warrant";

/// Glasses-local review inputs while a BRIDGE/FORGE warrant is focused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeGlassesReviewInput {
    /// Viture gaze dwell followed by a stable nod approves the warrant.
    VitureGazeDwellNod,
    /// Viture head shake rejects the warrant.
    VitureShake,
    /// Halo double tap approves the warrant.
    HaloDoubleTap,
    /// Halo long press rejects the warrant.
    HaloLongPress,
}

/// Standard warrant decision emitted by glasses review input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarrantReviewDecision {
    /// Approve the pending warrant gate.
    Approve,
    /// Reject the pending warrant gate.
    Reject,
}

impl WarrantReviewDecision {
    fn intent_kind(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
        }
    }
}

/// Returns the mission symbol used by BRIDGE/FORGE warrant decisions.
pub fn warrant_review_mission() -> Symbol {
    Symbol::qualified(
        BRIDGE_WARRANT_REVIEW_MISSION_NAMESPACE,
        BRIDGE_WARRANT_REVIEW_MISSION_NAME,
    )
}

/// Renders `packet` as the pinned Viture center-front review panel.
pub fn viture_warrant_review_panel(packet: &BridgePacket) -> Result<Expr> {
    let context = WarrantReviewContext::new(packet)?;
    let body = viture_review_card(packet, &context);
    let panel = sim_lib_scene::panel(
        VITURE_WARRANT_REVIEW_PANEL_ID,
        body,
        Anchor::new(AnchorSpace::Head, "center-front"),
        Transform3::new([0.0, 0.0, -1.2], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]),
    );
    let panel = mark_warrant_scene(panel, "warrant");
    validate("invalid Viture warrant review panel", &panel)?;
    Ok(panel)
}

/// Renders `packet` as the Viture spatial scene containing the review panel.
pub fn viture_warrant_review_scene(packet: &BridgePacket) -> Result<Expr> {
    let scene = sim_lib_scene::spatial(vec![viture_warrant_review_panel(packet)?]);
    validate("invalid Viture warrant review scene", &scene)?;
    Ok(scene)
}

/// Renders `packet` as a Halo `scene/glance` pager through the DEVICE_3 reducer.
pub fn halo_warrant_glance_pager(packet: &BridgePacket, profile: &DeviceProfile) -> Result<Expr> {
    WarrantReviewContext::new(packet)?;
    let source = halo_warrant_source_scene(packet);
    validate("invalid Halo warrant source scene", &source)?;
    let glance = GlanceReducer.reduce(&source, profile)?;
    validate("invalid Halo warrant glance pager", &glance)?;
    Ok(glance)
}

/// Builds the standard warrant decision Intent for `packet`.
pub fn warrant_review_intent(
    packet: &BridgePacket,
    decision: WarrantReviewDecision,
    origin: sim_lib_intent::Origin,
) -> Result<Expr> {
    let context = WarrantReviewContext::new(packet)?;
    let intent = sim_lib_intent::intent(
        decision.intent_kind(),
        origin,
        vec![
            ("mission", Expr::Symbol(warrant_review_mission())),
            ("packet-cid", build::text(context.packet_cid.to_owned())),
            ("warrant", warrant_expr(context.warrant)),
        ],
    );
    sim_lib_intent::validate_intent(&intent)
        .map_err(|error| Error::HostError(format!("invalid warrant review Intent: {error}")))?;
    Ok(intent)
}

/// Converts a glasses-local review input into a standard warrant decision Intent.
pub fn warrant_review_intent_from_glasses_input(
    packet: &BridgePacket,
    input: BridgeGlassesReviewInput,
    origin: sim_lib_intent::Origin,
) -> Result<Expr> {
    warrant_review_intent(packet, decision_for_input(input), origin)
}

pub(crate) fn glasses_warrant_scene_for_caps(
    packet: &BridgePacket,
    caps: &SurfaceCaps,
) -> Result<Option<Expr>> {
    if packet.warrant.is_none() {
        return Ok(None);
    }
    let profile = caps.device_profile();
    match sim_lib_view_device::glasses_class(&profile) {
        Some(GlassesClass::Stereo6Dof) => viture_warrant_review_scene(packet).map(Some),
        Some(GlassesClass::MonoHud) => halo_warrant_glance_pager(packet, &profile).map(Some),
        Some(GlassesClass::DisplayOnly) | None => Ok(None),
    }
}

fn decision_for_input(input: BridgeGlassesReviewInput) -> WarrantReviewDecision {
    match input {
        BridgeGlassesReviewInput::VitureGazeDwellNod | BridgeGlassesReviewInput::HaloDoubleTap => {
            WarrantReviewDecision::Approve
        }
        BridgeGlassesReviewInput::VitureShake | BridgeGlassesReviewInput::HaloLongPress => {
            WarrantReviewDecision::Reject
        }
    }
}

struct WarrantReviewContext<'a> {
    packet_cid: &'a str,
    warrant: &'a BridgeWarrant,
}

impl<'a> WarrantReviewContext<'a> {
    fn new(packet: &'a BridgePacket) -> Result<Self> {
        Ok(Self {
            packet_cid: packet.header.cid.as_deref().ok_or_else(|| {
                Error::Eval("BRIDGE/FORGE warrant review requires a stamped packet".to_owned())
            })?,
            warrant: packet.warrant.as_ref().ok_or_else(|| {
                Error::Eval("BRIDGE/FORGE warrant review requires a packet warrant".to_owned())
            })?,
        })
    }
}

fn viture_review_card(packet: &BridgePacket, context: &WarrantReviewContext<'_>) -> Expr {
    sim_lib_scene::node(
        "box",
        vec![
            ("role", build::sym("bridge-warrant-review")),
            ("title", build::text(REVIEW_TITLE)),
            ("status", build::sym("warrant")),
            ("warrant", Expr::Bool(true)),
            ("bypass-budget", Expr::Bool(true)),
            ("packet-cid", build::text(context.packet_cid.to_owned())),
            ("mission", Expr::Symbol(warrant_review_mission())),
            (
                "children",
                build::list(vec![
                    sim_lib_scene::badge("warrant", "FORGE gate"),
                    sim_lib_scene::text_node(format!(
                        "move {} from {}",
                        packet.header.move_kind.as_qualified_str(),
                        packet.header.from
                    )),
                    sim_lib_scene::text_node(format!(
                        "parts {} warrant parts {}",
                        packet.body.len(),
                        context.warrant.parts.len()
                    )),
                    sim_lib_scene::text_node(format!("packet {}", short_cid(context.packet_cid))),
                    decision_button("Approve", WarrantReviewDecision::Approve),
                    decision_button("Reject", WarrantReviewDecision::Reject),
                ]),
            ),
        ],
    )
}

fn halo_warrant_source_scene(packet: &BridgePacket) -> Expr {
    let mut source = sim_lib_scene::node(
        "stack",
        vec![
            ("dir", build::sym("column")),
            ("title", build::text(REVIEW_TITLE)),
            ("status", build::sym("critical")),
            ("warrant", Expr::Bool(true)),
            ("bypass-budget", Expr::Bool(true)),
            (
                "children",
                build::list(vec![sim_lib_scene::node(
                    "button",
                    vec![
                        ("label", build::text(HALO_GLYPH)),
                        ("target", Expr::Symbol(warrant_review_mission())),
                        ("control", build::sym("warrant-decision")),
                        (
                            "packet-cid",
                            build::text(packet.header.cid.clone().unwrap_or_default()),
                        ),
                    ],
                )]),
            ),
        ],
    );
    source = access::set(&source, "pager", build::sym("halo-glance"));
    source
}

fn decision_button(label: &str, decision: WarrantReviewDecision) -> Expr {
    sim_lib_scene::node(
        "button",
        vec![
            ("label", build::text(label)),
            ("target", Expr::Symbol(warrant_review_mission())),
            ("control", build::sym(decision.intent_kind())),
            (
                "intent-kind",
                Expr::Symbol(Symbol::qualified("intent", decision.intent_kind())),
            ),
        ],
    )
}

fn mark_warrant_scene(scene: Expr, status: &str) -> Expr {
    let scene = access::set(&scene, "status", build::sym(status));
    let scene = access::set(&scene, "warrant", Expr::Bool(true));
    let scene = access::set(&scene, "pinned", Expr::Bool(true));
    access::set(&scene, "bypass-budget", Expr::Bool(true))
}

fn warrant_expr(warrant: &BridgeWarrant) -> Expr {
    build::map(vec![
        ("moves", build::text(content_id_string(&warrant.moves))),
        ("frames", build::text(content_id_string(&warrant.frames))),
        (
            "parts",
            build::list(
                warrant
                    .parts
                    .iter()
                    .map(|(kind, cid)| {
                        build::map(vec![
                            ("part-kind", Expr::Symbol(kind.clone())),
                            ("cid", build::text(content_id_string(cid))),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn short_cid(cid: &str) -> &str {
    cid.char_indices()
        .nth(28)
        .map(|(index, _)| &cid[..index])
        .unwrap_or(cid)
}

fn validate(context: &str, scene: &Expr) -> Result<()> {
    sim_lib_scene::validate_scene(scene)
        .map_err(|error| Error::HostError(format!("{context}: {error}")))
}
