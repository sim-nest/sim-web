//! Run state: the live execution view of a topology.
//!
//! A run state accumulates execution events into per-node statuses, counters,
//! an ordered event log, and a set of live edges. It is fed by subscriptions to
//! a running topology (or, for replay, by a recorded event stream). It is plain
//! data the monitor view renders; it holds no second topology model.

use std::collections::{BTreeMap, BTreeSet};

use sim_kernel::Symbol;

/// A node's execution status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeStatus {
    /// Not yet started.
    Idle,
    /// Currently executing.
    Running,
    /// Completed successfully.
    Ok,
    /// Failed.
    Error,
}

impl NodeStatus {
    /// The status token (never color alone).
    pub fn token(self) -> &'static str {
        match self {
            NodeStatus::Idle => "idle",
            NodeStatus::Running => "running",
            NodeStatus::Ok => "ok",
            NodeStatus::Error => "error",
        }
    }
}

/// One execution event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunEvent {
    /// Logical time of the event.
    pub at: u64,
    /// The node the event concerns.
    pub node: Symbol,
    /// The event kind (`start`, `ok`, `error`, `route`, `log`).
    pub kind: Symbol,
    /// A human-readable message.
    pub message: String,
    /// For a `route` event, the edge `(from-node, to-node)` it traversed.
    pub edge: Option<(Symbol, Symbol)>,
}

impl RunEvent {
    /// A node lifecycle event.
    pub fn node(at: u64, node: &str, kind: &str, message: &str) -> Self {
        Self {
            at,
            node: Symbol::new(node),
            kind: Symbol::new(kind),
            message: message.to_owned(),
            edge: None,
        }
    }

    /// A routing event marking an edge live.
    pub fn route(at: u64, from: &str, to: &str) -> Self {
        Self {
            at,
            node: Symbol::new(from),
            kind: Symbol::new("route"),
            message: format!("{from} -> {to}"),
            edge: Some((Symbol::new(from), Symbol::new(to))),
        }
    }
}

/// The accumulated live state of a run.
#[derive(Clone, Debug, Default)]
pub struct RunState {
    /// Per-node status.
    pub statuses: BTreeMap<Symbol, NodeStatus>,
    /// Per-node event counter.
    pub counters: BTreeMap<Symbol, u64>,
    /// Ordered event log.
    pub events: Vec<RunEvent>,
    /// Currently live edges, by `(from-node, to-node)`.
    pub live_edges: BTreeSet<(Symbol, Symbol)>,
}

impl RunState {
    /// An empty run state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one event into the state.
    pub fn apply_event(&mut self, event: RunEvent) {
        *self.counters.entry(event.node.clone()).or_insert(0) += 1;
        match &*event.kind.name {
            "start" => {
                self.statuses
                    .insert(event.node.clone(), NodeStatus::Running);
            }
            "ok" => {
                self.statuses.insert(event.node.clone(), NodeStatus::Ok);
            }
            "error" => {
                self.statuses.insert(event.node.clone(), NodeStatus::Error);
            }
            "route" => {
                if let Some(edge) = &event.edge {
                    self.live_edges.insert(edge.clone());
                }
            }
            _ => {}
        }
        self.events.push(event);
    }

    /// The status of a node (Idle if unseen).
    pub fn status(&self, node: &Symbol) -> NodeStatus {
        self.statuses.get(node).copied().unwrap_or(NodeStatus::Idle)
    }

    /// The event count of a node.
    pub fn count(&self, node: &Symbol) -> u64 {
        self.counters.get(node).copied().unwrap_or(0)
    }

    /// Whether an edge is currently live.
    pub fn edge_live(&self, from: &Symbol, to: &Symbol) -> bool {
        self.live_edges.contains(&(from.clone(), to.clone()))
    }

    /// The events that concern a node, in order (for drill-down).
    pub fn events_for(&self, node: &Symbol) -> Vec<&RunEvent> {
        self.events
            .iter()
            .filter(|event| &event.node == node)
            .collect()
    }
}
