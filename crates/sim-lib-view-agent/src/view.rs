//! The topology composer view: a `Graph` encoded as a `scene/graph`.
//!
//! The view reads the existing `sim-lib-topology` `Graph` -- no second model --
//! and emits a graph Scene of agent nodes with typed ports, edges, and
//! role/capability labels. Node positions are read from graph metadata
//! (`pos:<id>`), so layout rides along as data on the same value.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};
use sim_lib_topology::{Graph, Node, Port};

/// The composer lens id.
pub const COMPOSER_LENS: &str = "view:agent-topology";

/// Encode a topology `Graph` into a `scene/graph` Scene.
pub fn composer_view(graph: &Graph) -> Expr {
    let nodes = graph.nodes.iter().map(|n| node_scene(graph, n)).collect();
    let edges = graph
        .edges
        .iter()
        .map(|edge| {
            node(
                "edge",
                vec![
                    ("from", port_pair(&edge.from.node, &edge.from.port)),
                    ("to", port_pair(&edge.to.node, &edge.to.port)),
                    ("status", sym("live")),
                ],
            )
        })
        .collect();
    node(
        "graph",
        vec![
            ("id", Expr::Symbol(graph.name.clone())),
            ("nodes", Expr::List(nodes)),
            ("edges", Expr::List(edges)),
            (
                "intents",
                data_map(vec![
                    ("node-move", sym("enabled")),
                    ("wire", sym("enabled")),
                    ("select", sym("enabled")),
                    ("create", sym("enabled")),
                    ("delete", sym("enabled")),
                ]),
            ),
        ],
    )
}

fn node_scene(graph: &Graph, n: &Node) -> Expr {
    let mut ports: Vec<Expr> = Vec::new();
    for port in &n.inputs {
        ports.push(port_descriptor(port, "in"));
    }
    for port in &n.outputs {
        ports.push(port_descriptor(port, "out"));
    }
    node(
        "node",
        vec![
            ("id", Expr::Symbol(n.id.as_symbol().clone())),
            ("title", Expr::String(n.id.as_symbol().name.to_string())),
            ("verb", Expr::Symbol(n.verb.clone())),
            (
                "role",
                n.role.clone().map(Expr::Symbol).unwrap_or(Expr::Nil),
            ),
            ("at", position(graph, n.id.as_symbol())),
            ("ports", Expr::List(ports)),
        ],
    )
}

fn port_descriptor(port: &Port, dir: &str) -> Expr {
    data_map(vec![
        ("id", Expr::Symbol(port.name.clone())),
        ("dir", sym(dir)),
        ("label", Expr::String(port.name.name.to_string())),
        ("required", Expr::Bool(port.required)),
    ])
}

fn port_pair(node_id: &sim_lib_topology::NodeId, port: &Symbol) -> Expr {
    Expr::Vector(vec![
        Expr::Symbol(node_id.as_symbol().clone()),
        Expr::Symbol(port.clone()),
    ])
}

fn position(graph: &Graph, node_id: &Symbol) -> Expr {
    let key = Symbol::new(format!("pos:{}", node_id.name));
    graph
        .metadata
        .iter()
        .find_map(|(meta_key, value)| (meta_key == &key).then(|| value.clone()))
        .unwrap_or_else(|| data_map(vec![("x", Expr::Nil), ("y", Expr::Nil)]))
}
