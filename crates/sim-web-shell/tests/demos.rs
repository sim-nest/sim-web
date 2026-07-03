//! Headless golden-Scene demos: one fixture per domain lens.
//!
//! Each demo builds a fixture value, renders it through its domain lens, and
//! asserts the Scene is valid, deterministic (a golden property: rendering is a
//! pure function of the value), and carries the domain's signature scene node
//! kind. No browser is needed; the emitted Scene is data.

use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, NumberLiteral, Symbol};

use sim_value::build::sym;

fn number(value: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::new("i64"),
        canonical: value.to_owned(),
    })
}

/// Assert a Scene is valid, deterministic, and carries `signature`.
fn assert_golden(scene: &Expr, rerender: &Expr, signature: &str) {
    sim_lib_scene::validate_scene(scene).expect("the demo scene is valid");
    assert_eq!(
        scene, rerender,
        "rendering is deterministic (golden property)"
    );
    assert!(
        sim_test_support::contains_kind(scene, signature),
        "the demo carries its signature kind '{signature}'"
    );
}

fn agent_fixture() -> sim_lib_topology::Graph {
    use sim_lib_topology::{Edge, EdgeId, Graph, Node, NodeId, PortRef};
    let mut graph = Graph::minimal("demo-flow");
    graph.nodes.push(Node::named(NodeId::new("planner"), "in"));
    graph.nodes.push(Node::named(NodeId::new("writer"), "out"));
    graph.edges.push(Edge::new(
        EdgeId(1),
        PortRef::output(NodeId::new("planner")),
        PortRef::input(NodeId::new("writer")),
    ));
    graph
}

#[test]
fn agent_topology_demo() {
    let graph = agent_fixture();
    let scene = sim_lib_view_agent::composer_view(&graph);
    assert_golden(&scene, &sim_lib_view_agent::composer_view(&graph), "graph");
}

#[test]
fn article_demo() {
    use sim_lib_view_doc::{article, embed_block, equation, prose, section};
    let doc = article(
        "Demo",
        vec![
            section("Intro"),
            prose("body"),
            equation("E = mc^2"),
            embed_block(sym("result"), "view:default"),
        ],
    );
    let scene = sim_lib_view_doc::article_formatted(&doc);
    assert_golden(&scene, &sim_lib_view_doc::article_formatted(&doc), "embed");
}

#[test]
fn math_demo() {
    let scene = sim_lib_view_math::plot_view("y=x", &[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]);
    let rerender = sim_lib_view_math::plot_view("y=x", &[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]);
    assert_golden(&scene, &rerender, "plot");
}

#[test]
fn daw_demo() {
    use sim_lib_daw_session::{DawClip, DawSession, DawTrack};
    let mut session = DawSession::new("demo", "Demo Session", 48_000).unwrap();
    session
        .add_track(
            DawTrack::audio("lead", "Lead", 2)
                .unwrap()
                .with_clip(DawClip::silence("c1", 0, 48_000).unwrap()),
        )
        .unwrap();
    let scene = sim_lib_view_daw::daw_view(&session);
    assert_golden(&scene, &sim_lib_view_daw::daw_view(&session), "timeline");
}

#[test]
fn performance_keyboard_demo() {
    let scene = sim_lib_view_daw::performance_keyboard_demo_scene();
    assert_golden(
        &scene,
        &sim_lib_view_daw::performance_keyboard_demo_scene(),
        "keyboard",
    );
    assert!(contains_symbol(
        &scene,
        "music/player-chain",
        "onscreen-keyboard"
    ));
    assert!(contains_symbol(&scene, "audio-synth/instrument", "dx7"));
}

#[test]
fn performance_workbench_demo() {
    let scene = sim_lib_view_daw::performance_workbench_demo_scene();
    assert_golden(
        &scene,
        &sim_lib_view_daw::performance_workbench_demo_scene(),
        "piano-roll",
    );
    assert!(sim_test_support::contains_kind(&scene, "keyboard"));
    assert!(sim_test_support::contains_kind(&scene, "player-rack"));
    assert!(contains_symbol(
        &scene,
        "music/player-chain",
        "onscreen-keyboard"
    ));
}

#[test]
fn arranger_object_roll_demo() {
    let scene = sim_lib_view_daw::arranger_object_roll_demo_scene();
    assert_golden(
        &scene,
        &sim_lib_view_daw::arranger_object_roll_demo_scene(),
        "object-roll",
    );
    assert!(contains_symbol(&scene, "music/arranger", "song-a"));
}

fn codec_cx() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::register_core_classes(&mut cx);
    let lisp = sim_codec_lisp::LispCodecLib::new(cx.registry_mut().fresh_codec_id()).unwrap();
    cx.load_lib(&lisp).unwrap();
    let json = sim_codec_json::JsonCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&json).unwrap();
    let binary = sim_codec_binary::BinaryCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&binary).unwrap();
    let algol = sim_codec_algol::AlgolCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&algol).unwrap();
    cx
}

#[test]
fn codec_demo() {
    let mut cx = codec_cx();
    let codecs: Vec<Symbol> = ["lisp", "json", "binary", "algol"]
        .iter()
        .map(|n| Symbol::qualified("codec", *n))
        .collect();
    let value = Expr::Map(vec![(sym("a"), number("1"))]);
    let scene = sim_lib_view_codec::multi_codec_view(&mut cx, &codecs, &value);
    let rerender = sim_lib_view_codec::multi_codec_view(&mut cx, &codecs, &value);
    assert_golden(&scene, &rerender, "embed");
}

fn contains_symbol(expr: &Expr, namespace: &str, name: &str) -> bool {
    match expr {
        Expr::Symbol(symbol)
            if symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name =>
        {
            true
        }
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) | Expr::Block(items) => items
            .iter()
            .any(|item| contains_symbol(item, namespace, name)),
        Expr::Map(entries) => entries.iter().any(|(key, value)| {
            contains_symbol(key, namespace, name) || contains_symbol(value, namespace, name)
        }),
        Expr::Call { operator, args } => {
            contains_symbol(operator, namespace, name)
                || args.iter().any(|arg| contains_symbol(arg, namespace, name))
        }
        Expr::Infix { left, right, .. } => {
            contains_symbol(left, namespace, name) || contains_symbol(right, namespace, name)
        }
        Expr::Prefix { arg, .. } | Expr::Postfix { arg, .. } => {
            contains_symbol(arg, namespace, name)
        }
        Expr::Quote { expr, .. } => contains_symbol(expr, namespace, name),
        Expr::Annotated { expr, annotations } => {
            contains_symbol(expr, namespace, name)
                || annotations
                    .iter()
                    .any(|(_, value)| contains_symbol(value, namespace, name))
        }
        Expr::Extension { payload, .. } => contains_symbol(payload, namespace, name),
        _ => false,
    }
}
