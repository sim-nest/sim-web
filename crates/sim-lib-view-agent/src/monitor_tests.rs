//! Tests for the live monitor, drill-down, and replay.

use sim_kernel::Symbol;
use sim_lib_topology::{Edge, EdgeId, Graph, Node, NodeId, PortRef};

use crate::monitor::{monitor_view, node_detail};
use crate::replay::{replay, replay_final};
use crate::run::{NodeStatus, RunEvent, RunState};

fn graph() -> Graph {
    let mut graph = Graph::minimal("flow");
    graph.nodes.push(Node::named(NodeId::new("source"), "in"));
    graph.nodes.push(Node::named(NodeId::new("sink"), "out"));
    graph.edges.push(Edge::new(
        EdgeId(1),
        PortRef::output(NodeId::new("source")),
        PortRef::input(NodeId::new("sink")),
    ));
    graph
}

fn recorded_run() -> Vec<RunEvent> {
    vec![
        RunEvent::node(1, "source", "start", "begin"),
        RunEvent::node(2, "source", "ok", "produced packet"),
        RunEvent::route(3, "source", "sink"),
        RunEvent::node(4, "sink", "start", "consuming"),
        RunEvent::node(5, "sink", "ok", "done"),
    ]
}

#[test]
fn the_monitor_renders_status_badges_counters_and_a_timeline() {
    let graph = graph();
    let run = replay_final(&recorded_run());
    let scene = monitor_view(&graph, &run);
    sim_lib_scene::validate_scene(&scene).expect("the monitor scene is valid");
    assert_eq!(run.status(&Symbol::new("sink")), NodeStatus::Ok);
    assert!(run.edge_live(&Symbol::new("source"), &Symbol::new("sink")));
    assert_eq!(run.count(&Symbol::new("source")), 3);
}

#[test]
fn drilling_into_a_node_shows_its_events() {
    let run = replay_final(&recorded_run());
    let detail = node_detail(&run, &Symbol::new("source"));
    sim_lib_scene::validate_scene(&detail).expect("node detail is a valid scene");
    assert_eq!(run.events_for(&Symbol::new("source")).len(), 3);
    assert_eq!(run.events_for(&Symbol::new("sink")).len(), 2);
}

#[test]
fn a_past_run_replays_visually_frame_by_frame() {
    let graph = graph();
    let events = recorded_run();
    let frames = replay(&graph, &events);
    assert_eq!(
        frames.len(),
        events.len() + 1,
        "one frame per step plus the initial frame"
    );
    for frame in &frames {
        sim_lib_scene::validate_scene(frame).expect("every replay frame is a valid scene");
    }
    // The final frame equals rendering the fully-folded state.
    let final_state = replay_final(&events);
    assert_eq!(*frames.last().unwrap(), monitor_view(&graph, &final_state));
}

#[test]
fn live_streaming_updates_status_incrementally() {
    let mut run = RunState::new();
    assert_eq!(run.status(&Symbol::new("source")), NodeStatus::Idle);
    run.apply_event(RunEvent::node(1, "source", "start", "begin"));
    assert_eq!(run.status(&Symbol::new("source")), NodeStatus::Running);
    run.apply_event(RunEvent::node(2, "source", "error", "boom"));
    assert_eq!(run.status(&Symbol::new("source")), NodeStatus::Error);
}
