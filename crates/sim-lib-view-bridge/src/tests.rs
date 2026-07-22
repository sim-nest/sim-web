use sim_codec_bridge::{
    BridgeFramePayload, BridgeHeader, BridgePacket, BridgePart, BridgePatchPayload,
    BridgeProvenance, BridgeReceiptPayload, BridgeReviewPayload, BridgeScore, BridgeVotePayload,
    packet_to_expr, stamp_packet_cid,
};
use sim_kernel::{Expr, Symbol, testing::eager_cx as cx};
use sim_lib_intent::Origin;
use sim_lib_view::SurfaceCodec;
use sim_value::access::field;
use sim_value::build::{entry, int, list, map, text};

use crate::{
    BridgePacketSurfaceCodec, bridge_packet_edit, bridge_packet_view, patch_edit_intent,
    receipt_edit_intent, review_edit_intent, vote_edit_intent,
};

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

fn edit_intent(target: Expr, path: Expr, value: Expr) -> Expr {
    sim_lib_intent::intent(
        "edit-field",
        Origin::human(9),
        vec![("target", target), ("path", path), ("value", value)],
    )
}

fn collab_path(action: &str) -> Expr {
    list(vec![text("bridge-collab"), text(action)])
}

fn walk(expr: &Expr, visit: &mut impl FnMut(&Expr)) {
    visit(expr);
    match expr {
        Expr::Map(entries) => {
            for (_, value) in entries {
                walk(value, visit);
            }
        }
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => {
            for item in items {
                walk(item, visit);
            }
        }
        _ => {}
    }
}

fn kind_count(expr: &Expr, name: &str) -> usize {
    let mut count = 0;
    walk(expr, &mut |item| {
        if matches!(
            field(item, "kind"),
            Some(Expr::Symbol(symbol))
                if symbol.namespace.as_deref() == Some("scene") && symbol.name.as_ref() == name
        ) {
            count += 1;
        }
    });
    count
}

fn role_count(expr: &Expr, role: &str) -> usize {
    let mut count = 0;
    walk(expr, &mut |item| {
        if matches!(
            field(item, "role"),
            Some(Expr::Symbol(symbol)) if symbol.name.as_ref() == role
        ) {
            count += 1;
        }
    });
    count
}

#[test]
fn bridge_packet_surface_codec_encodes_a_valid_scene() {
    let mut cx = cx();
    let codec = BridgePacketSurfaceCodec::new();
    let value = packet_to_expr(&packet());
    let caps = sim_lib_view::surface::SurfaceCaps::from_preset("desktop", "test").unwrap();
    let scene = codec.encode(&mut cx, &value, &caps).unwrap();

    sim_lib_scene::validate_scene(&scene).unwrap();
}

#[test]
fn bridge_packet_view_renders_valid_scene() {
    let mut cx = cx();
    let caps = sim_lib_view::surface::SurfaceCaps::from_preset("desktop", "test").unwrap();
    let scene = bridge_packet_view(&mut cx, &packet(), &caps).unwrap();

    sim_lib_scene::validate_scene(&scene).unwrap();
}

#[test]
fn bridge_packet_view_renders_collaboration_forms() {
    let mut cx = cx();
    let caps = sim_lib_view::surface::SurfaceCaps::from_preset("desktop", "test").unwrap();
    let scene = bridge_packet_view(&mut cx, &packet(), &caps).unwrap();

    assert_eq!(role_count(&scene, "edit-form"), 4);
    assert!(kind_count(&scene, "field") >= 10);
    assert_eq!(kind_count(&scene, "button"), 4);
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

#[test]
fn bridge_packet_decode_rejects_non_edit_or_unscoped_intents() {
    let mut cx = cx();
    let codec = BridgePacketSurfaceCodec::new();
    let packet = packet();
    let value = packet_to_expr(&packet);
    let tap = sim_lib_intent::intent(
        "tap",
        Origin::human(9),
        vec![
            ("target", Expr::Symbol(Symbol::new("bridge-packet"))),
            ("control", Expr::Symbol(Symbol::new("submit"))),
        ],
    );
    let empty_path = edit_intent(
        Expr::Symbol(Symbol::new("bridge-packet")),
        list(vec![]),
        Expr::String("free value".to_owned()),
    );
    let wrong_path = edit_intent(
        Expr::Symbol(Symbol::new("bridge-packet")),
        list(vec![text("body"), text("O1")]),
        map(vec![("target", text("body/O1/payload"))]),
    );
    let wrong_target = edit_intent(
        Expr::Symbol(Symbol::new("not-the-packet")),
        collab_path("review"),
        map(vec![
            ("target", text("body/O1/payload")),
            ("body", text("looks correct")),
        ]),
    );

    assert!(codec.decode(&mut cx, &value, &tap).is_err());
    assert!(codec.decode(&mut cx, &value, &empty_path).is_err());
    assert!(codec.decode(&mut cx, &value, &wrong_path).is_err());
    assert!(codec.decode(&mut cx, &value, &wrong_target).is_err());
}

#[test]
fn bridge_packet_decode_accepts_explicit_packet_cid_target() {
    let mut cx = cx();
    let packet = packet();
    let target = Expr::String(packet.header.cid.clone().unwrap());
    let part = bridge_packet_edit(
        &mut cx,
        &packet,
        &edit_intent(
            target,
            collab_path("review"),
            map(vec![
                ("target", text("body/O1/payload")),
                ("body", text("looks correct")),
            ]),
        ),
    )
    .unwrap();

    assert_eq!(part_kind(&part), Symbol::qualified("bridge", "Review"));
}

#[test]
fn browser_shaped_vote_and_receipt_values_become_valid_parts() {
    let mut cx = cx();
    let packet = packet();
    let vote = bridge_packet_edit(
        &mut cx,
        &packet,
        &edit_intent(
            Expr::String(packet.header.cid.clone().unwrap()),
            collab_path("vote"),
            map(vec![
                ("target", text("body/O1/payload")),
                (
                    "scores",
                    Expr::Vector(vec![map(vec![
                        ("axis", text("correctness")),
                        ("value", int(1)),
                        ("reason", text("checked packet shape")),
                    ])]),
                ),
            ]),
        ),
    )
    .unwrap();
    let receipt = bridge_packet_edit(
        &mut cx,
        &packet,
        &edit_intent(
            Expr::String(packet.header.cid.clone().unwrap()),
            collab_path("receipt"),
            map(vec![
                ("status", text("accepted")),
                ("refs", list(vec![text("body/O1/payload")])),
            ]),
        ),
    )
    .unwrap();

    assert_eq!(part_kind(&vote), Symbol::qualified("bridge", "Vote"));
    assert_eq!(part_kind(&receipt), Symbol::qualified("bridge", "Receipt"));
    BridgeVotePayload::from_expr(part_payload(&vote)).unwrap();
    BridgeReceiptPayload::from_expr(part_payload(&receipt)).unwrap();
}

#[test]
fn bridge_packet_decode_validates_generated_collaboration_payloads() {
    let mut cx = cx();
    let packet = packet();
    let invalid_vote = bridge_packet_edit(
        &mut cx,
        &packet,
        &edit_intent(
            Expr::Symbol(Symbol::new("bridge-packet")),
            collab_path("vote"),
            map(vec![
                ("target", text("body/O1/payload")),
                ("scores", Expr::Vector(vec![])),
            ]),
        ),
    );
    let invalid_receipt = bridge_packet_edit(
        &mut cx,
        &packet,
        &edit_intent(
            Expr::Symbol(Symbol::new("bridge-packet")),
            collab_path("receipt"),
            map(vec![
                ("status", text("")),
                ("refs", list(vec![text("body/O1/payload")])),
            ]),
        ),
    );

    assert!(invalid_vote.is_err());
    assert!(invalid_receipt.is_err());
}
