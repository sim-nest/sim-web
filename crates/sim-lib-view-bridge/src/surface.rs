//! Reversible BRIDGE packet review surface for SIM Web.
//!
//! The surface renders one `BridgePacket` as Scene data and decodes ordinary
//! `intent/edit-field` values into typed BRIDGE collaboration parts. Human and
//! agent operators edit the same packet expression: a patch, review, vote, or
//! receipt is a BRIDGE part record, not a separate browser-side protocol.

mod controls;

use sim_codec_bridge::{
    BridgeBook, BridgePacket, BridgePatchPayload, BridgeReceiptPayload, BridgeReviewPayload,
    BridgeScore, BridgeVotePayload, expr_to_packet, packet_to_expr, validate_collab_payload,
};
use sim_kernel::{Cx, Error, Expr, Result, Symbol};
use sim_lib_view::codec::reduce_for_caps;
use sim_lib_view::{Draft, Operation, SurfaceCaps, SurfaceCodec};
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
        require_intent_kind(intent, "edit-field")?;
        let packet = expr_to_packet(value)?;
        require_packet_target(&packet, intent)?;
        let path = path_segments(intent)?;
        let action = match path.as_slice() {
            [lane, action] if lane == "bridge-collab" => action.as_str(),
            _ => {
                return Err(Error::Eval(
                    "BRIDGE edit must target bridge-collab action".to_owned(),
                ));
            }
        };
        let part = collaboration_part(&packet, action, required_field(intent, "value")?)?;
        validate_bridge_part(&part)?;
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
    children.push(controls::collaboration_controls(packet));
    let scene = sim_lib_scene::stack("column", children);
    sim_lib_scene::validate_scene(&scene)
        .map_err(|error| Error::HostError(format!("invalid BRIDGE packet scene: {error}")))?;
    Ok(scene)
}

fn collaboration_part(packet: &BridgePacket, action: &str, value: &Expr) -> Result<Expr> {
    match action {
        "patch" => {
            reject_unknown_fields(value, &["target", "replacement"], "bridge/Patch edit")?;
            let patch = BridgePatchPayload::new(
                packet_cid(packet)?,
                required_non_empty_string(value, "target")?,
                required_field(value, "replacement")?.clone(),
            );
            Ok(part_expr("P1", "Patch", patch.to_expr()))
        }
        "review" => {
            reject_unknown_fields(value, &["target", "body"], "bridge/Review edit")?;
            let review = BridgeReviewPayload::new(
                required_non_empty_string(value, "target")?,
                required_non_empty_string(value, "body")?,
            );
            Ok(part_expr("R1", "Review", review.to_expr()))
        }
        "vote" => {
            reject_unknown_fields(value, &["target", "scores"], "bridge/Vote edit")?;
            let vote =
                BridgeVotePayload::new(required_non_empty_string(value, "target")?, scores(value)?);
            Ok(part_expr("V1", "Vote", vote.to_expr()))
        }
        "receipt" => {
            reject_unknown_fields(value, &["status", "refs"], "bridge/Receipt edit")?;
            let receipt =
                BridgeReceiptPayload::new(required_symbol_like(value, "status")?, refs(value)?);
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
        .map(score)
        .collect::<Result<Vec<_>>>()?;
    if scores.is_empty() {
        return Err(Error::Eval(
            "BRIDGE packet vote edit requires at least one score".to_owned(),
        ));
    }
    Ok(scores)
}

fn score(value: &Expr) -> Result<BridgeScore> {
    if let Ok(score) = BridgeScore::from_expr(value) {
        return Ok(score);
    }
    reject_unknown_fields(value, &["axis", "value", "reason"], "bridge/Score edit")?;
    Ok(BridgeScore::new(
        required_symbol_like(value, "axis")?,
        required_i64_like(value, "value")?,
        required_non_empty_string(value, "reason")?,
    ))
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

fn require_intent_kind(intent: &Expr, expected: &str) -> Result<()> {
    let kind = sim_lib_intent::intent_kind_of(intent)
        .ok_or_else(|| Error::Eval("intent is missing kind".to_owned()))?;
    let expected_kind = sim_lib_intent::intent_kind(expected);
    if kind != expected_kind {
        return Err(Error::Eval(format!("expected intent/{expected}")));
    }
    Ok(())
}

fn require_packet_target(packet: &BridgePacket, intent: &Expr) -> Result<()> {
    let target = required_field(intent, "target")?;
    if target_matches(target, "bridge-packet")
        || packet
            .header
            .cid
            .as_deref()
            .is_some_and(|cid| target_matches(target, cid))
    {
        return Ok(());
    }
    Err(Error::Eval(
        "BRIDGE edit target must be bridge-packet or the packet cid".to_owned(),
    ))
}

fn target_matches(target: &Expr, expected: &str) -> bool {
    match target {
        Expr::String(value) => value == expected,
        Expr::Symbol(symbol) => symbol.as_qualified_str() == expected,
        _ => false,
    }
}

fn validate_bridge_part(part: &Expr) -> Result<()> {
    let kind = required_symbol(part, "kind")?;
    let payload = required_field(part, "payload")?;
    validate_collab_payload(kind, payload)
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

fn required_non_empty_string<'a>(expr: &'a Expr, name: &str) -> Result<&'a str> {
    let value = required_string(expr, name)?;
    if value.trim().is_empty() {
        return Err(Error::Eval(format!("field {name} must not be empty")));
    }
    Ok(value)
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

fn required_symbol_like(expr: &Expr, name: &str) -> Result<Symbol> {
    match required_field(expr, name)? {
        Expr::Symbol(value) => Ok(value.clone()),
        Expr::String(value) if !value.trim().is_empty() => Ok(Symbol::new(value.clone())),
        Expr::String(_) => Err(Error::Eval(format!("field {name} must not be empty"))),
        _ => Err(Error::TypeMismatch {
            expected: "symbol or string",
            found: "non-symbol",
        }),
    }
}

fn required_i64_like(expr: &Expr, name: &str) -> Result<i64> {
    match required_field(expr, name)? {
        Expr::Number(number) => number
            .canonical
            .parse()
            .map_err(|_| Error::Eval(format!("field {name} must be an i64 literal"))),
        Expr::String(value) => value
            .parse()
            .map_err(|_| Error::Eval(format!("field {name} must be an i64 literal"))),
        _ => Err(Error::TypeMismatch {
            expected: "number or numeric string",
            found: "non-number",
        }),
    }
}

fn required_vector<'a>(expr: &'a Expr, name: &str) -> Result<&'a [Expr]> {
    match required_field(expr, name)? {
        Expr::List(items) | Expr::Vector(items) => Ok(items),
        _ => Err(Error::Eval(format!("field {name} must be a list"))),
    }
}

fn reject_unknown_fields(expr: &Expr, allowed: &[&str], label: &str) -> Result<()> {
    let Expr::Map(fields) = expr else {
        return Err(Error::Eval(format!("{label} must be a map")));
    };
    for (key, _) in fields {
        let Some(name) = field_name(key) else {
            return Err(Error::Eval(format!("{label} field keys must be symbols")));
        };
        if !allowed.contains(&name.as_str()) {
            return Err(Error::Eval(format!("unknown {label} field {name}")));
        }
    }
    Ok(())
}

fn field_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Symbol(symbol) => Some(symbol.name.to_string()),
        Expr::String(value) => Some(value.clone()),
        _ => None,
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
