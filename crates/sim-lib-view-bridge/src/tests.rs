use std::sync::Arc;

use sim_codec_bridge::{
    BridgeFramePayload, BridgeHeader, BridgePacket, BridgePart, BridgePatchPayload,
    BridgeProvenance, BridgeReceiptPayload, BridgeReviewPayload, BridgeScore, BridgeVotePayload,
    packet_to_expr, stamp_packet_cid,
};
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_lib_intent::Origin;
use sim_lib_view::{SurfaceCodec, roundtrip_holds};
use sim_value::access::field;
use sim_value::build::entry;

use crate::{
    BridgePacketSurfaceCodec, bridge_packet_edit, bridge_packet_view, patch_edit_intent,
    receipt_edit_intent, review_edit_intent, vote_edit_intent,
};

fn cx() -> Cx {
    Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
}

fn packet() -> BridgePacket {
    stamp_packet_cid(&BridgePacket {
        header: BridgeHeader {
            cid: None,
            move_kind: Symbol::new("reply"),
            from: "model:drafter".to_owned(),
            to: vec!["human:reviewer".to_owned(), "model:judge".to_owned()],
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
                    entry("codec", Expr::Symbol(Symbol::qualified("codec", "bridge"))),
                    entry("shape", Expr::Symbol(Symbol::qualified("core", "Map"))),
                ]),
            },
        ],
        warrant: None,
    })
    .unwrap()
}

fn part_payload(part: &Expr) -> &Expr {
    field(part, "payload").unwrap()
}

fn part_kind(part: &Expr) -> Symbol {
    match field(part, "kind").unwrap() {
        Expr::Symbol(symbol) => symbol.clone(),
        other => panic!("unexpected part kind {other:?}"),
    }
}

#[test]
fn bridge_packet_surface_roundtrip_holds() {
    let mut cx = cx();
    let codec = BridgePacketSurfaceCodec::new();
    let value = packet_to_expr(&packet());

    assert!(roundtrip_holds(&mut cx, &codec, &value).unwrap());
}

#[test]
fn bridge_packet_view_renders_valid_scene() {
    let mut cx = cx();
    let caps = sim_lib_view::surface::SurfaceCaps::from_preset("desktop", "test").unwrap();
    let scene = bridge_packet_view(&mut cx, &packet(), &caps).unwrap();

    sim_lib_scene::validate_scene(&scene).unwrap();
}

#[test]
fn human_patch_and_model_patch_decode_identically() {
    let mut cx = cx();
    let packet = packet();
    let human = bridge_packet_edit(
        &mut cx,
        &packet,
        &patch_edit_intent(
            "body/O1/payload",
            Expr::String("accepted answer".to_owned()),
            Origin::human(1),
        ),
    )
    .unwrap();
    let model = bridge_packet_edit(
        &mut cx,
        &packet,
        &patch_edit_intent(
            "body/O1/payload",
            Expr::String("accepted answer".to_owned()),
            Origin::agent(1),
        ),
    )
    .unwrap();
    let human_patch = BridgePatchPayload::from_expr(part_payload(&human)).unwrap();
    let model_patch = BridgePatchPayload::from_expr(part_payload(&model)).unwrap();

    assert_eq!(part_kind(&human), Symbol::qualified("bridge", "Patch"));
    assert_eq!(human_patch, model_patch);
}

#[test]
fn edits_decode_to_typed_review_vote_and_receipt_parts() {
    let mut cx = cx();
    let packet = packet();
    let review = bridge_packet_edit(
        &mut cx,
        &packet,
        &review_edit_intent("body/O1/payload", "looks correct", Origin::human(2)),
    )
    .unwrap();
    let vote = bridge_packet_edit(
        &mut cx,
        &packet,
        &vote_edit_intent(
            "body/O1/payload",
            vec![BridgeScore::new(
                Symbol::new("correctness"),
                1,
                "keeps the packet valid",
            )],
            Origin::agent(3),
        ),
    )
    .unwrap();
    let receipt = bridge_packet_edit(
        &mut cx,
        &packet,
        &receipt_edit_intent(
            Symbol::new("accepted"),
            vec!["body/O1/payload".to_owned()],
            Origin::agent(4),
        ),
    )
    .unwrap();

    assert_eq!(part_kind(&review), Symbol::qualified("bridge", "Review"));
    assert_eq!(part_kind(&vote), Symbol::qualified("bridge", "Vote"));
    assert_eq!(part_kind(&receipt), Symbol::qualified("bridge", "Receipt"));
    assert_eq!(
        BridgeReviewPayload::from_expr(part_payload(&review))
            .unwrap()
            .target,
        "body/O1/payload"
    );
    assert_eq!(
        BridgeVotePayload::from_expr(part_payload(&vote))
            .unwrap()
            .scores[0]
            .axis,
        Symbol::new("correctness")
    );
    assert_eq!(
        BridgeReceiptPayload::from_expr(part_payload(&receipt))
            .unwrap()
            .status,
        Symbol::new("accepted")
    );
}

#[test]
fn direct_codec_decode_uses_same_packet_value() {
    let mut cx = cx();
    let codec = BridgePacketSurfaceCodec::new();
    let packet = packet();
    let value = packet_to_expr(&packet);
    let draft = codec
        .decode(
            &mut cx,
            &value,
            &patch_edit_intent(
                "body/O1/payload",
                Expr::String("accepted answer".to_owned()),
                Origin::human(5),
            ),
        )
        .unwrap();
    let op = codec.commit(&mut cx, &draft).unwrap();

    assert_eq!(draft.base, value);
    assert!(matches!(op.form, Expr::Map(_)));
}
