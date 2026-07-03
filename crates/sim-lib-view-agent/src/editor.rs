//! The composer editor: composer Intents applied to a `Graph`.
//!
//! Each composer Intent (create/move/wire/unwire/delete) maps to one or more
//! `sim-lib-topology` `PatchOp`s and is applied through `apply_topology_patch_ops`,
//! producing a new `Graph`. There is no second model: the topology patch engine
//! is the one that mutates topologies. Because the Intent already carries an
//! `origin.operator`, a human and an agent edit the same topology through the
//! same path; only the recorded operator differs.

use sim_kernel::{CapabilityName, Cx, Error, Expr, Result, Symbol};
use sim_lib_intent::{field, intent_kind_of};
use sim_lib_topology::{
    Edge, EdgeId, Graph, Node, NodeId, PatchOp, PortRef, TopologyPatch, apply_topology_patch_ops,
};

/// Apply a composer Intent to `graph`, returning the new graph.
///
/// Editing a topology is a checked operation: the caller must hold the
/// `topology-write` capability, gated exactly like any other writer. The
/// composer applies structural ops without recompiling, because a topology
/// under construction is legitimately incomplete; full validation runs at save
/// or run.
pub fn apply_composer_intent(cx: &mut Cx, graph: &Graph, intent: &Expr) -> Result<Graph> {
    cx.require(&CapabilityName::new("topology-write"))?;
    let ops = composer_ops(graph, intent)?;
    let patch = TopologyPatch { ops };
    apply_topology_patch_ops(graph, &patch)
}

fn composer_ops(graph: &Graph, intent: &Expr) -> Result<Vec<PatchOp>> {
    let kind = intent_kind_of(intent)
        .ok_or_else(|| Error::HostError("composer input is not an Intent".to_owned()))?;
    match &*kind.name {
        "create" => {
            let verb = require_symbol(intent, "class")?;
            let id = create_id(graph, intent, &verb);
            let mut ops = vec![PatchOp::AddNode(Node::named(
                NodeId(id.clone()),
                verb.name.to_string(),
            ))];
            if let Some(at) = field(intent, "at") {
                ops.push(PatchOp::SetMetadata {
                    key: pos_key(&id),
                    value: at.clone(),
                });
            }
            Ok(ops)
        }
        "move" => {
            let node = require_symbol(intent, "node")?;
            let at = field(intent, "at").cloned().unwrap_or(Expr::Nil);
            Ok(vec![PatchOp::SetMetadata {
                key: pos_key(&node),
                value: at,
            }])
        }
        "wire" => {
            let from = port_ref(field(intent, "from"))?;
            let to = port_ref(field(intent, "to"))?;
            Ok(vec![PatchOp::AddEdge {
                edge: Edge::new(EdgeId(0), from, to),
                explicit_id: false,
            }])
        }
        "unwire" => {
            let edge = field(intent, "edge")
                .ok_or_else(|| Error::HostError("unwire is missing an 'edge'".to_owned()))?;
            let from = port_ref(sub_field(edge, "from"))?;
            let to = port_ref(sub_field(edge, "to"))?;
            Ok(vec![PatchOp::RemoveEdge { from, to }])
        }
        "delete" => {
            let targets = match field(intent, "targets") {
                Some(Expr::List(items)) => items,
                _ => {
                    return Err(Error::HostError(
                        "delete 'targets' must be a list".to_owned(),
                    ));
                }
            };
            targets
                .iter()
                .map(|target| match target {
                    Expr::Symbol(symbol) => Ok(PatchOp::RemoveNode(NodeId(symbol.clone()))),
                    _ => Err(Error::HostError(
                        "delete target must be a node id".to_owned(),
                    )),
                })
                .collect()
        }
        other => Err(Error::HostError(format!(
            "composer does not handle intent '{other}'"
        ))),
    }
}

fn create_id(graph: &Graph, intent: &Expr, verb: &Symbol) -> Symbol {
    if let Some(args) = field(intent, "args")
        && let Some(Expr::Symbol(id)) = sub_field(args, "id")
    {
        return id.clone();
    }
    Symbol::new(format!("{}{}", verb.name, graph.nodes.len()))
}

fn pos_key(node: &Symbol) -> Symbol {
    Symbol::new(format!("pos:{}", node.name))
}

fn port_ref(field: Option<&Expr>) -> Result<PortRef> {
    let map = field.ok_or_else(|| Error::HostError("missing a port reference".to_owned()))?;
    let node = match sub_field(map, "node") {
        Some(Expr::Symbol(symbol)) => symbol.clone(),
        _ => {
            return Err(Error::HostError(
                "port ref 'node' must be a symbol".to_owned(),
            ));
        }
    };
    let port = match sub_field(map, "port") {
        Some(Expr::Symbol(symbol)) => symbol.clone(),
        _ => Symbol::new("out"),
    };
    Ok(PortRef::new(NodeId(node), port))
}

fn require_symbol(intent: &Expr, name: &str) -> Result<Symbol> {
    match field(intent, name) {
        Some(Expr::Symbol(symbol)) => Ok(symbol.clone()),
        _ => Err(Error::HostError(format!(
            "composer intent field '{name}' must be a symbol"
        ))),
    }
}

fn sub_field<'a>(map: &'a Expr, name: &str) -> Option<&'a Expr> {
    let Expr::Map(entries) = map else {
        return None;
    };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(symbol) if &*symbol.name == name && symbol.namespace.is_none())
            .then_some(value)
    })
}
