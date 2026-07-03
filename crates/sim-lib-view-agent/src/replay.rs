//! Replay: reconstruct a past run visually from a recorded event stream.
//!
//! Because the monitor Scene is a pure function of the topology and the run
//! state, replaying a recorded event stream reproduces the exact sequence of
//! Scenes the operator saw live. Each frame is a Scene value, so a replay is
//! itself data -- snapshottable, diffable, and testable headlessly.

use sim_kernel::Expr;
use sim_lib_topology::Graph;

use crate::monitor::monitor_view;
use crate::run::{RunEvent, RunState};

/// Replay a recorded event stream over `graph`, returning one monitor Scene per
/// step (including the initial empty frame). The final frame is the end state.
pub fn replay(graph: &Graph, events: &[RunEvent]) -> Vec<Expr> {
    let mut run = RunState::new();
    let mut frames = Vec::with_capacity(events.len() + 1);
    frames.push(monitor_view(graph, &run));
    for event in events {
        run.apply_event(event.clone());
        frames.push(monitor_view(graph, &run));
    }
    frames
}

/// Reconstruct just the final run state from a recorded event stream.
pub fn replay_final(events: &[RunEvent]) -> RunState {
    let mut run = RunState::new();
    for event in events {
        run.apply_event(event.clone());
    }
    run
}
