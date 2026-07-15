//! Reversible BRIDGE packet review surface for SIM Web.
//!
//! The surface renders one `BridgePacket` as Scene data and decodes ordinary
//! `intent/edit-field` values into typed BRIDGE collaboration parts. Human and
//! agent operators edit the same packet expression: a patch, review, vote, or
//! receipt is a BRIDGE part record, not a separate browser-side protocol.

use sim_codec_bridge::{
    BridgeBook, BridgePacket, BridgePatchPayload, BridgeReceiptPayload, BridgeReviewPayload,
    BridgeScore, BridgeVotePayload, expr_to_packet, packet_to_expr,
};
use sim_kernel::{Cx, Error, Expr, Result, Symbol};
use sim_lib_view::codec::reduce_for_caps;
use sim_lib_view::{Draft, Operation, SurfaceCaps, SurfaceCodec, roundtrip_holds};
use sim_value::access::field;
use sim_value::build::{entry, list, map, text};

/// Stable id for the BRIDGE packet review surface codec.
pub const BRIDGE_PACKET_SURFACE_CODEC_ID: &str = "surface:bridge-packet";

/// Reversible surface codec for BRIDGE packet review.
#[derive(Clone, Copy, Debug, Default)]
pub struct BridgePacketSurfaceCodec;

impl BridgePacketSurfaceCodec {
    /// Builds the codec.
    pub fn new() -> Self {
        Self
    }
}

impl SurfaceCodec for BridgePacketSurfaceCodec {
    fn encode(&self, _cx: &mut Cx, value: &Expr, caps: &SurfaceCaps) -> Result<Expr> {
        let packet = expr_to_packet(value)?;
        let scene = packet_scene(&packet, caps)?;
        Ok(reduce_for_caps(&scene, caps))
    }

    fn decode(&self, _cx: &mut Cx, value: &Expr, intent: &Expr) -> Result<Draft> {
        sim_lib_intent::validate_intent(intent)
            .map_err(|error| Error::HostError(format!("invalid intent: {error}")))?;
        let packet = expr_to_packet(value)?;
        let path = path_segments(intent)?;
        let target = required_field(intent, "value")?;
        if path.is_empty() {
            return Ok(Draft::clean(value.clone(), target.clone()));
        }
        if path.first().map(String::as_str) != Some("bridge-collab") {
            return Err(Error::Eval(
                "BRIDGE packet surface only decodes bridge-collab edits".to_owned(),
            ));
        }
        let action = path.get(1).map(String::as_str).ok_or_else(|| {
            Error::Eval("BRIDGE collaboration edit is missing an action".to_owned())
        })?;
        let part = collaboration_part(&packet, action, target)?;
        Ok(Draft::clean(value.clone(), part))
    }

    fn commit(&self, _cx: &mut Cx, draft: &Draft) -> Result<Operation> {
        Ok(Operation {
            form: map(vec![
                (
                    "op",
                    Expr::Symbol(Symbol::qualified("bridge", "surface-edit")),
                ),
                ("value", draft.proposed.clone()),
            ]),
        })
    }
}

/// Renders `packet` through the BRIDGE packet surface.
pub fn bridge_packet_view(cx: &mut Cx, packet: &BridgePacket, caps: &SurfaceCaps) -> Result<Expr> {
    BridgePacketSurfaceCodec::new().encode(cx, &packet_to_expr(packet), caps)
}

/// Decodes one edit intent against `packet` into a typed BRIDGE part record.
pub fn bridge_packet_edit(cx: &mut Cx, packet: &BridgePacket, intent: &Expr) -> Result<Expr> {
    let value = packet_to_expr(packet);
    let codec = BridgePacketSurfaceCodec::new();
    debug_assert!(roundtrip_holds(cx, &codec, &value).unwrap_or(false));
    let draft = codec.decode(cx, &value, intent)?;
    if !draft.committable {
        return Err(Error::Eval(
            "BRIDGE packet edit produced a rejected draft".to_owned(),
        ));
    }
    Ok(draft.proposed)
}

fn packet_scene(packet: &BridgePacket, caps: &SurfaceCaps) -> Result<Expr> {
    let profiles = BridgeBook::standard()
        .profiles
        .matching_profiles(packet)
        .into_iter()
        .map(|profile| profile.as_qualified_str())
        .collect::<Vec<_>>()
        .join(", ");
    let cid = packet.header.cid.as_deref().unwrap_or("unstamped");
    let mut children = vec![
        sim_lib_scene::badge("surface", BRIDGE_PACKET_SURFACE_CODEC_ID),
        sim_lib_scene::build::text_node(format!("cid {cid}")),
        sim_lib_scene::build::text_node(format!(
            "move {} from {}",
            packet.header.move_kind.as_qualified_str(),
            packet.header.from
        )),
        sim_lib_scene::build::text_node(format!(
            "profiles {}",
            if profiles.is_empty() {
                "none".to_owned()
            } else {
                profiles
            }
        )),
        sim_lib_scene::build::text_node(format!("surface {}", caps.preset_name())),
    ];
    children.extend(packet.body.iter().map(|part| {
        sim_lib_scene::box_(
            "part",
            vec![
                sim_lib_scene::badge("kind", &part.kind.as_qualified_str()),
                sim_lib_scene::build::text_node(format!("id {}", part.id.as_qualified_str())),
                sim_lib_scene::build::text_node(payload_summary(&part.payload)),
            ],
        )
    }));
    let scene = sim_lib_scene::stack("column", children);
    sim_lib_scene::validate_scene(&scene)
        .map_err(|error| Error::HostError(format!("invalid BRIDGE packet scene: {error}")))?;
    Ok(scene)
}

fn collaboration_part(packet: &BridgePacket, action: &str, value: &Expr) -> Result<Expr> {
    match action {
        "patch" => {
            let patch = BridgePatchPayload::new(
                packet_cid(packet)?,
                required_string(value, "target")?,
                required_field(value, "replacement")?.clone(),
            );
            Ok(part_expr("P1", "Patch", patch.to_expr()))
        }
        "review" => {
            let review = BridgeReviewPayload::new(
                required_string(value, "target")?,
                required_string(value, "body")?,
            );
            Ok(part_expr("R1", "Review", review.to_expr()))
        }
        "vote" => {
            let vote = BridgeVotePayload::new(required_string(value, "target")?, scores(value)?);
            Ok(part_expr("V1", "Vote", vote.to_expr()))
        }
        "receipt" => {
            let receipt =
                BridgeReceiptPayload::new(required_symbol(value, "status")?.clone(), refs(value)?);
            Ok(part_expr("Rc1", "Receipt", receipt.to_expr()))
        }
        other => Err(Error::Eval(format!(
            "unknown BRIDGE collaboration edit action {other}"
        ))),
    }
}

fn part_expr(id: &str, kind: &str, payload: Expr) -> Expr {
    Expr::Map(vec![
        entry("id", Expr::Symbol(Symbol::new(id))),
        entry("kind", Expr::Symbol(Symbol::qualified("bridge", kind))),
        entry("payload", payload),
    ])
}

fn scores(value: &Expr) -> Result<Vec<BridgeScore>> {
    let scores = required_vector(value, "scores")?
        .iter()
        .map(BridgeScore::from_expr)
        .collect::<Result<Vec<_>>>()?;
    if scores.is_empty() {
        return Err(Error::Eval(
            "BRIDGE packet vote edit requires at least one score".to_owned(),
        ));
    }
    Ok(scores)
}

fn refs(value: &Expr) -> Result<Vec<String>> {
    required_vector(value, "refs")?
        .iter()
        .map(|item| match item {
            Expr::String(value) => Ok(value.clone()),
            _ => Err(Error::TypeMismatch {
                expected: "string",
                found: "non-string",
            }),
        })
        .collect()
}

fn packet_cid(packet: &BridgePacket) -> Result<String> {
    packet.header.cid.clone().ok_or_else(|| {
        Error::Eval("BRIDGE packet surface edits require a stamped packet".to_owned())
    })
}

fn path_segments(intent: &Expr) -> Result<Vec<String>> {
    required_vector(intent, "path")?
        .iter()
        .map(|segment| match segment {
            Expr::String(value) => Ok(value.clone()),
            Expr::Symbol(symbol) => Ok(symbol.name.to_string()),
            _ => Err(Error::Eval(
                "BRIDGE packet edit path segments must be strings or symbols".to_owned(),
            )),
        })
        .collect()
}

fn required_field<'a>(expr: &'a Expr, name: &str) -> Result<&'a Expr> {
    field(expr, name).ok_or_else(|| Error::Eval(format!("missing field {name}")))
}

fn required_string<'a>(expr: &'a Expr, name: &str) -> Result<&'a str> {
    match required_field(expr, name)? {
        Expr::String(value) => Ok(value),
        _ => Err(Error::TypeMismatch {
            expected: "string",
            found: "non-string",
        }),
    }
}

fn required_symbol<'a>(expr: &'a Expr, name: &str) -> Result<&'a Symbol> {
    match required_field(expr, name)? {
        Expr::Symbol(value) => Ok(value),
        _ => Err(Error::TypeMismatch {
            expected: "symbol",
            found: "non-symbol",
        }),
    }
}

fn required_vector<'a>(expr: &'a Expr, name: &str) -> Result<&'a [Expr]> {
    match required_field(expr, name)? {
        Expr::List(items) | Expr::Vector(items) => Ok(items),
        _ => Err(Error::Eval(format!("field {name} must be a list"))),
    }
}

fn payload_summary(payload: &Expr) -> String {
    let rendered = format!("{payload:?}");
    if rendered.len() <= 96 {
        rendered
    } else {
        format!("{}...", &rendered[..96])
    }
}

/// Builds a patch edit intent for the packet review surface.
pub fn patch_edit_intent(target: &str, replacement: Expr, origin: sim_lib_intent::Origin) -> Expr {
    sim_lib_intent::intent(
        "edit-field",
        origin,
        vec![
            ("target", Expr::Symbol(Symbol::new("bridge-packet"))),
            ("path", list(vec![text("bridge-collab"), text("patch")])),
            (
                "value",
                map(vec![("target", text(target)), ("replacement", replacement)]),
            ),
        ],
    )
}

/// Builds a review edit intent for the packet review surface.
pub fn review_edit_intent(target: &str, body: &str, origin: sim_lib_intent::Origin) -> Expr {
    sim_lib_intent::intent(
        "edit-field",
        origin,
        vec![
            ("target", Expr::Symbol(Symbol::new("bridge-packet"))),
            ("path", list(vec![text("bridge-collab"), text("review")])),
            (
                "value",
                map(vec![("target", text(target)), ("body", text(body))]),
            ),
        ],
    )
}

/// Builds a vote edit intent for the packet review surface.
pub fn vote_edit_intent(
    target: &str,
    scores: Vec<BridgeScore>,
    origin: sim_lib_intent::Origin,
) -> Expr {
    sim_lib_intent::intent(
        "edit-field",
        origin,
        vec![
            ("target", Expr::Symbol(Symbol::new("bridge-packet"))),
            ("path", list(vec![text("bridge-collab"), text("vote")])),
            (
                "value",
                map(vec![
                    ("target", text(target)),
                    (
                        "scores",
                        Expr::Vector(scores.iter().map(BridgeScore::to_expr).collect()),
                    ),
                ]),
            ),
        ],
    )
}

/// Builds a receipt edit intent for the packet review surface.
pub fn receipt_edit_intent(
    status: Symbol,
    refs: Vec<String>,
    origin: sim_lib_intent::Origin,
) -> Expr {
    sim_lib_intent::intent(
        "edit-field",
        origin,
        vec![
            ("target", Expr::Symbol(Symbol::new("bridge-packet"))),
            ("path", list(vec![text("bridge-collab"), text("receipt")])),
            (
                "value",
                map(vec![
                    ("status", Expr::Symbol(status)),
                    ("refs", list(refs.into_iter().map(text).collect::<Vec<_>>())),
                ]),
            ),
        ],
    )
}
