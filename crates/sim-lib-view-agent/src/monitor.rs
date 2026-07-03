//! The live monitor view: a topology Graph plus its run state.
//!
//! The monitor renders the same topology as the composer, with status badges on
//! nodes, per-node counters, edge liveness, and a `scene/timeline` of execution
//! events. Drilling into a node yields its event log. The run state streams in
//! through subscriptions; the same state replays a past run (see `replay`).

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{data_map, node, sym};
use sim_lib_topology::Graph;
use sim_value::build::uint;

use crate::run::RunState;

/// The monitor lens id.
pub const MONITOR_LENS: &str = "view:agent-monitor";

/// Render a topology and its run state into a live monitor Scene.
pub fn monitor_view(graph: &Graph, run: &RunState) -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("monitor")),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![monitored_graph(graph, run), timeline(run)]),
            ),
        ],
    )
}

fn monitored_graph(graph: &Graph, run: &RunState) -> Expr {
    let nodes = graph
        .nodes
        .iter()
        .map(|n| {
            let id = n.id.as_symbol();
            let status = run.status(id);
            node(
                "node",
                vec![
                    ("id", Expr::Symbol(id.clone())),
                    ("title", Expr::String(id.name.to_string())),
                    (
                        "status",
                        node(
                            "badge",
                            vec![
                                ("status", sym(status.token())),
                                ("label", Expr::String(status.token().to_owned())),
                            ],
                        ),
                    ),
                    ("count", uint(run.count(id))),
                ],
            )
        })
        .collect();
    let edges = graph
        .edges
        .iter()
        .map(|edge| {
            let from = edge.from.node.as_symbol();
            let to = edge.to.node.as_symbol();
            let live = run.edge_live(from, to);
            node(
                "edge",
                vec![
                    ("from", Expr::Symbol(from.clone())),
                    ("to", Expr::Symbol(to.clone())),
                    ("status", sym(if live { "live" } else { "idle" })),
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
        ],
    )
}

fn timeline(run: &RunState) -> Expr {
    let markers = run
        .events
        .iter()
        .map(|event| {
            // Use `event` rather than `kind` here: `kind` is the scene-node tag,
            // and an execution event marker is plain data, not a scene node.
            // `data_map` enforces that reserved-key guard.
            data_map(vec![
                ("at", uint(event.at)),
                ("node", Expr::Symbol(event.node.clone())),
                ("event", Expr::Symbol(event.kind.clone())),
                ("label", Expr::String(event.message.clone())),
            ])
        })
        .collect();
    node(
        "timeline",
        vec![("lane", sym("execution")), ("events", Expr::List(markers))],
    )
}

/// Drill down from a node into its event log.
pub fn node_detail(run: &RunState, node_id: &Symbol) -> Expr {
    let rows = run
        .events_for(node_id)
        .into_iter()
        .map(|event| {
            node(
                "text",
                vec![(
                    "text",
                    Expr::String(format!("@{} {} {}", event.at, event.kind, event.message)),
                )],
            )
        })
        .collect();
    node(
        "box",
        vec![
            ("role", sym("node-detail")),
            ("node", Expr::Symbol(node_id.clone())),
            ("children", Expr::List(rows)),
        ],
    )
}
