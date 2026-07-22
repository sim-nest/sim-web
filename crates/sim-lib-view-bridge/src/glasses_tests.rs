use std::sync::Arc;

use sim_codec_bridge::{
    BridgeBook, BridgeFramePayload, BridgeHeader, BridgePacket, BridgePart, BridgeProvenance,
    packet_to_expr, stamp_packet_cid, warrant_for_packet,
};
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_lib_intent::{Origin, field, intent_kind_of, validate_intent};
use sim_lib_scene::GlanceCard;
use sim_lib_view::{SurfaceCaps, SurfaceCodec};
use sim_lib_view_device::{DeviceSurfaceCapsExt, GlassesClass};
use sim_lib_view_spatial::{AttentionBudget, rank_glasses};
use sim_value::access;
use sim_value::build::{entry, qsym};

use crate::{
    BridgeGlassesReviewInput, BridgePacketSurfaceCodec, VITURE_WARRANT_REVIEW_PANEL_ID,
    WarrantReviewDecision, halo_warrant_glance_pager, viture_warrant_review_panel,
    warrant_review_intent, warrant_review_intent_from_glasses_input, warrant_review_mission,
};

fn cx() -> Cx {
    Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
}

fn packet_with_warrant() -> BridgePacket {
    let mut packet = BridgePacket {
        header: BridgeHeader {
            cid: None,
            move_kind: Symbol::new("reply"),
            from: "model:drafter".to_owned(),
            to: vec!["human:reviewer".to_owned()],
            role: Symbol::new("implementer"),
            parents: vec!["core/sha256-bridge-v1:root".to_owned()],
            task: Symbol::new("T1"),
            output: Symbol::new("O1"),
            ceiling: Vec::new(),
            context: Vec::new(),
            provenance: BridgeProvenance::default(),
        },
        body: vec![
            BridgePart {
                id: Symbol::new("T1"),
                kind: Symbol::qualified("bridge", "Frame"),
                payload: BridgeFramePayload::new(Symbol::qualified("bridge", "answer")).to_expr(),
            },
            BridgePart {
                id: Symbol::new("O1"),
                kind: Symbol::qualified("bridge", "Return"),
                payload: Expr::Map(vec![
                    entry("codec", qsym("codec", "bridge")),
                    entry("shape", qsym("core", "Map")),
                ]),
            },
        ],
        warrant: None,
    };
    packet.warrant = Some(warrant_for_packet(&BridgeBook::standard(), &packet).unwrap());
    stamp_packet_cid(&packet).unwrap()
}

#[test]
fn warrant_approved_on_both_glasses() {
    let packet = packet_with_warrant();
    let value = packet_to_expr(&packet);
    let codec = BridgePacketSurfaceCodec::new();
    let mut cx = cx();

    let viture_caps = SurfaceCaps::from_preset("glasses-luma-ultra", "viture.review").unwrap();
    let viture_scene = codec.encode(&mut cx, &value, &viture_caps).unwrap();
    assert_scene_kind(&viture_scene, "spatial");
    let viture_panel = first_child(&viture_scene);
    assert_scene_kind(viture_panel, "panel");
    assert_eq!(
        access::field_str(viture_panel, "id"),
        Some(VITURE_WARRANT_REVIEW_PANEL_ID)
    );
    assert_eq!(
        access::field_bool(viture_panel, "bypass-budget"),
        Some(true)
    );
    assert_eq!(access::field_bool(viture_panel, "warrant"), Some(true));

    let ranked = rank_glasses(
        &viture_scene,
        GlassesClass::Stereo6Dof,
        [1.0, 0.0, 0.0],
        &AttentionBudget::new(0),
    )
    .unwrap();
    let ranked_panel = first_child(&ranked);
    assert_eq!(
        access::field_bool(ranked_panel, "attention-lit"),
        Some(true)
    );
    assert_eq!(
        access::field_bool(ranked_panel, "attention-pinned"),
        Some(true)
    );

    let halo_caps = SurfaceCaps::from_preset("glasses-hud", "halo.review").unwrap();
    let halo_scene = codec.encode(&mut cx, &value, &halo_caps).unwrap();
    assert_scene_kind(&halo_scene, sim_lib_scene::GLANCE_KIND);
    let halo_card = GlanceCard::from_scene(&halo_scene).unwrap();
    assert_eq!(halo_card.title, "BRIDGE/FORGE warrant");
    assert_eq!(halo_card.urgency, "critical");
    assert!(halo_card.bypass_budget);
    assert_eq!(halo_card.action.unwrap().label, "OK/X");

    let viture_approve = warrant_review_intent_from_glasses_input(
        &packet,
        BridgeGlassesReviewInput::VitureGazeDwellNod,
        Origin::human(1),
    )
    .unwrap();
    let halo_approve = warrant_review_intent_from_glasses_input(
        &packet,
        BridgeGlassesReviewInput::HaloDoubleTap,
        Origin::human(2),
    )
    .unwrap();

    assert_intent_kind(&viture_approve, "approve");
    assert_intent_kind(&halo_approve, "approve");
    assert_eq!(
        field(&viture_approve, "mission"),
        Some(&Expr::Symbol(warrant_review_mission()))
    );
    assert_eq!(
        field(&halo_approve, "packet-cid"),
        Some(&Expr::String(packet.header.cid.clone().unwrap()))
    );
    validate_intent(&viture_approve).unwrap();
    validate_intent(&halo_approve).unwrap();
}

#[test]
fn warrant_review_rejects_on_shake_or_long_press() {
    let packet = packet_with_warrant();
    let viture_reject = warrant_review_intent_from_glasses_input(
        &packet,
        BridgeGlassesReviewInput::VitureShake,
        Origin::human(3),
    )
    .unwrap();
    let halo_reject = warrant_review_intent_from_glasses_input(
        &packet,
        BridgeGlassesReviewInput::HaloLongPress,
        Origin::human(4),
    )
    .unwrap();

    assert_intent_kind(&viture_reject, "reject");
    assert_intent_kind(&halo_reject, "reject");
    validate_intent(&viture_reject).unwrap();
    validate_intent(&halo_reject).unwrap();
}

#[test]
fn warrant_review_requires_warrant_and_stamp() {
    let mut packet = packet_with_warrant();
    packet.warrant = None;
    assert!(viture_warrant_review_panel(&packet).is_err());

    let mut unstamped = packet_with_warrant();
    unstamped.header.cid = None;
    assert!(
        warrant_review_intent(&unstamped, WarrantReviewDecision::Approve, Origin::human(5))
            .is_err()
    );
}

#[test]
fn halo_pager_uses_device_profile_reducer() {
    let packet = packet_with_warrant();
    let profile = SurfaceCaps::from_preset("glasses-hud", "halo.review")
        .unwrap()
        .device_profile();

    let glance = halo_warrant_glance_pager(&packet, &profile).unwrap();

    assert_scene_kind(&glance, sim_lib_scene::GLANCE_KIND);
    let card = GlanceCard::from_scene(&glance).unwrap();
    assert_eq!(
        card.action.unwrap().target,
        Expr::Symbol(warrant_review_mission())
    );
    assert!(card.bypass_budget);
}

fn assert_intent_kind(intent: &Expr, expected: &str) {
    assert_eq!(
        intent_kind_of(intent).map(|symbol| symbol.name.to_string()),
        Some(expected.to_owned())
    );
}

fn assert_scene_kind(scene: &Expr, expected: &str) {
    let kind = sim_lib_scene::node_kind(scene).expect("scene kind");
    assert_eq!(kind.namespace.as_deref(), Some("scene"));
    assert_eq!(kind.name.as_ref(), expected);
}

fn first_child(scene: &Expr) -> &Expr {
    match access::field(scene, "children").expect("children") {
        Expr::List(children) => children.first().expect("first child"),
        other => panic!("children must be a list, got {other:?}"),
    }
}
