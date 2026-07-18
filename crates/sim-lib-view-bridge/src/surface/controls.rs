use sim_codec_bridge::BridgePacket;
use sim_kernel::{Expr, Symbol};
use sim_value::build::{int, list, text};

pub(super) fn collaboration_controls(packet: &BridgePacket) -> Expr {
    let target = packet_target(packet);
    let packet_path = default_packet_path(packet);
    sim_lib_scene::box_(
        "bridge-collab",
        vec![
            action_form(
                "patch",
                target.clone(),
                vec![
                    field_node(
                        "target",
                        "Patch target",
                        &packet_path,
                        vec![text("target")],
                        "string",
                    ),
                    field_node(
                        "replacement",
                        "Replacement",
                        "",
                        vec![text("replacement")],
                        "string",
                    ),
                ],
            ),
            action_form(
                "review",
                target.clone(),
                vec![
                    field_node(
                        "target",
                        "Review target",
                        &packet_path,
                        vec![text("target")],
                        "string",
                    ),
                    field_node("body", "Review", "", vec![text("body")], "string"),
                ],
            ),
            action_form(
                "vote",
                target.clone(),
                vec![
                    field_node(
                        "target",
                        "Vote target",
                        &packet_path,
                        vec![text("target")],
                        "string",
                    ),
                    field_node(
                        "axis",
                        "Score axis",
                        "correctness",
                        vec![text("scores"), int(0), text("axis")],
                        "symbol",
                    ),
                    field_node(
                        "value",
                        "Score",
                        "1",
                        vec![text("scores"), int(0), text("value")],
                        "number",
                    ),
                    field_node(
                        "reason",
                        "Score reason",
                        "",
                        vec![text("scores"), int(0), text("reason")],
                        "string",
                    ),
                ],
            ),
            action_form(
                "receipt",
                target,
                vec![
                    field_node(
                        "status",
                        "Receipt status",
                        "accepted",
                        vec![text("status")],
                        "symbol",
                    ),
                    field_node(
                        "ref",
                        "Receipt ref",
                        &packet_path,
                        vec![text("refs"), int(0)],
                        "string",
                    ),
                ],
            ),
        ],
    )
}

fn packet_target(packet: &BridgePacket) -> Expr {
    packet
        .header
        .cid
        .as_ref()
        .map(text)
        .unwrap_or_else(|| Expr::Symbol(Symbol::new("bridge-packet")))
}

fn default_packet_path(packet: &BridgePacket) -> String {
    format!("body/{}/payload", packet.header.output.as_qualified_str())
}

fn action_form(action: &str, target: Expr, children: Vec<Expr>) -> Expr {
    let mut nodes = children;
    nodes.push(sim_lib_scene::node(
        "button",
        vec![
            ("label", text(format!("Create {action}"))),
            ("control", text("submit")),
        ],
    ));
    sim_lib_scene::node(
        "box",
        vec![
            ("role", Expr::Symbol(Symbol::new("edit-form"))),
            ("target", target),
            ("path", list(vec![text("bridge-collab"), text(action)])),
            ("value-codec", text("codec:bridge")),
            ("children", list(nodes)),
        ],
    )
}

fn field_node(
    name: &str,
    label: &str,
    value: &str,
    value_path: Vec<Expr>,
    value_kind: &str,
) -> Expr {
    sim_lib_scene::node(
        "field",
        vec![
            ("name", text(name)),
            ("label", text(label)),
            ("value", text(value)),
            ("value-path", list(value_path)),
            ("value-kind", text(value_kind)),
            ("required", Expr::Bool(true)),
        ],
    )
}
