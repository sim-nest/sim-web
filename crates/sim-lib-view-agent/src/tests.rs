//! Tests for the topology composer: build, edit, save/load, and the shared bus.

use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr};
use sim_lib_intent::{Origin, intent};
use sim_lib_topology::Graph;

use crate::editor::apply_composer_intent;
use crate::persist::{composer_load, composer_save};
use crate::view::composer_view;

fn cx() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::register_core_classes(&mut cx);
    // Editing a topology is a checked operation; the composer operator holds the
    // topology-write capability (gated exactly like any other caller).
    cx.grant_named("topology-write");
    cx
}

use sim_value::build::sym;

fn create(origin: Origin, id: &str, verb: &str) -> Expr {
    intent(
        "create",
        origin,
        vec![
            ("class", sym(verb)),
            (
                "at",
                Expr::Map(vec![(sym("x"), sym("0")), (sym("y"), sym("0"))]),
            ),
            ("args", Expr::Map(vec![(sym("id"), sym(id))])),
        ],
    )
}

fn port(node: &str, port: &str) -> Expr {
    Expr::Map(vec![(sym("node"), sym(node)), (sym("port"), sym(port))])
}

/// Build a valid source -> sink topology graphically.
fn built_graph(cx: &mut Cx) -> Graph {
    let mut graph = Graph::minimal("flow");
    graph = apply_composer_intent(cx, &graph, &create(Origin::human(1), "source", "in")).unwrap();
    graph = apply_composer_intent(cx, &graph, &create(Origin::human(2), "sink", "out")).unwrap();
    let wire = intent(
        "wire",
        Origin::human(3),
        vec![("from", port("source", "out")), ("to", port("sink", "in"))],
    );
    apply_composer_intent(cx, &graph, &wire).unwrap()
}

#[test]
fn a_human_builds_a_topology_graphically() {
    let mut cx = cx();
    let graph = built_graph(&mut cx);
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.edges.len(), 1);
    let scene = composer_view(&graph);
    sim_lib_scene::validate_scene(&scene).expect("the composer scene is valid");
}

#[test]
fn a_topology_saves_and_loads_as_sim_data() {
    let mut cx = cx();
    let graph = built_graph(&mut cx);
    let saved = composer_save(&cx, &graph);
    let reloaded = composer_load(&mut cx, saved.clone()).unwrap();
    let saved_again = composer_save(&cx, &reloaded);
    assert_eq!(saved, saved_again, "a topology round-trips as SIM data");
    assert_eq!(reloaded.nodes.len(), 2);
    assert_eq!(reloaded.edges.len(), 1);
}

#[test]
fn an_agent_edits_the_same_topology_a_human_built() {
    let mut cx = cx();
    let graph = built_graph(&mut cx);
    // An agent issues a wire Intent on the same bus, rerouting the same graph:
    // it inserts a reviewer node and connects it. Both operators edit one graph.
    let graph = apply_composer_intent(
        &mut cx,
        &graph,
        &create(Origin::agent(10), "reviewer", "wire"),
    )
    .unwrap();
    let agent_wire = intent(
        "wire",
        Origin::agent(11),
        vec![
            ("from", port("source", "out")),
            ("to", port("reviewer", "in")),
        ],
    );
    let graph = apply_composer_intent(&mut cx, &graph, &agent_wire).unwrap();
    assert_eq!(
        graph.nodes.len(),
        3,
        "the agent's node joins the human's graph"
    );
    assert!(
        graph
            .nodes
            .iter()
            .any(|n| n.id.as_symbol().name.as_ref() == "reviewer"),
        "the agent-created node is present in the shared topology"
    );
    // The human sees it live: the rendered scene includes the new node.
    let scene = composer_view(&graph);
    sim_lib_scene::validate_scene(&scene).unwrap();
}

#[test]
fn move_and_delete_edit_the_topology() {
    let mut cx = cx();
    let mut graph = built_graph(&mut cx);
    // Move records a position in graph metadata.
    let mv = intent(
        "move",
        Origin::human(20),
        vec![
            ("node", sym("planner")),
            (
                "at",
                Expr::Map(vec![(sym("x"), sym("120")), (sym("y"), sym("80"))]),
            ),
        ],
    );
    graph = apply_composer_intent(&mut cx, &graph, &mv).unwrap();
    assert!(
        graph
            .metadata
            .iter()
            .any(|(key, _)| key.name.as_ref() == "pos:planner"),
        "move stores the node position as graph metadata"
    );

    // Add a scratch node, then delete it.
    graph = apply_composer_intent(
        &mut cx,
        &graph,
        &create(Origin::human(21), "scratch", "wire"),
    )
    .unwrap();
    assert!(
        graph
            .nodes
            .iter()
            .any(|n| n.id.as_symbol().name.as_ref() == "scratch")
    );
    let del = intent(
        "delete",
        Origin::human(22),
        vec![("targets", Expr::List(vec![sym("scratch")]))],
    );
    graph = apply_composer_intent(&mut cx, &graph, &del).unwrap();
    assert!(
        !graph
            .nodes
            .iter()
            .any(|n| n.id.as_symbol().name.as_ref() == "scratch"),
        "delete removes the node"
    );
}
