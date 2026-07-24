//! Proof tests for SurfaceCodec-backed bridge sessions.

// conformance: SurfaceCodec bridge sessions encode, decode, commit, project,
// reject stale optimistic revisions, and isolate session state.

use sim_kernel::testing::eager_cx as cx;
use sim_kernel::{Expr, NumberLiteral, Symbol};
use sim_lib_intent::{Origin, intent};
use sim_lib_view::{LensRegistry, UNIVERSAL_SURFACE_CODEC_ID, register_universal_default, surface};
use sim_value::build::keyword as sym;

use crate::fixture::FixtureTransport;
use crate::session::Session;
use crate::transport::Transport;

fn registry() -> LensRegistry {
    let mut registry = LensRegistry::new();
    register_universal_default(&mut registry, false);
    registry
}

fn number(value: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: sym("i64"),
        canonical: value.to_owned(),
    })
}

fn doc() -> Expr {
    Expr::Map(vec![
        (Expr::Symbol(sym("a")), number("1")),
        (Expr::Symbol(sym("b")), number("2")),
    ])
}

fn edit_a_to(value: Expr, new_value: Expr) -> Expr {
    intent(
        "edit-field",
        Origin::human(1),
        vec![
            ("target", value),
            (
                "path",
                Expr::List(vec![Expr::Vector(vec![
                    Expr::Symbol(sym("k")),
                    Expr::Symbol(sym("a")),
                ])]),
            ),
            ("value", new_value),
        ],
    )
}

fn edit_a_to_9() -> Expr {
    edit_a_to(doc(), number("9"))
}

fn surface_codec() -> Symbol {
    Symbol::new(UNIVERSAL_SURFACE_CODEC_ID)
}

fn child_count(scene: &Expr) -> usize {
    let Expr::Map(entries) = scene else {
        return 0;
    };
    entries
        .iter()
        .find_map(|(key, value)| {
            let is_children = matches!(key, Expr::Symbol(s) if &*s.name == "children");
            match value {
                Expr::List(items) if is_children => Some(items.len()),
                _ => None,
            }
        })
        .unwrap_or(0)
}

#[test]
fn codec_session_encodes_decodes_commits_and_pumps() {
    let mut cx = cx();
    let registry = registry();
    let transport = FixtureTransport::new().with(sym("doc"), doc());
    let mut session = Session::new(transport);

    let initial = session
        .open_codec(
            &mut cx,
            &registry,
            sym("pane-1"),
            sym("doc"),
            surface_codec(),
            surface::preset("desktop").unwrap(),
        )
        .unwrap();
    sim_lib_scene::validate_scene(&initial).expect("initial scene is valid");

    session
        .submit_intent(&mut cx, &registry, &sym("pane-1"), &edit_a_to_9())
        .unwrap();

    let updates = session.pump(&mut cx, &registry).unwrap();
    assert_eq!(updates.len(), 1);
    let rebuilt = sim_lib_scene::apply(&initial, &updates[0].diff).unwrap();
    assert_eq!(rebuilt, updates[0].scene);
    assert_eq!(
        sim_value::access::field(
            &session.transport_mut().read(&mut cx, &sym("doc")).unwrap(),
            "a"
        ),
        Some(&number("9"))
    );
}

#[test]
fn optimistic_revision_failure_requires_a_fresh_pump() {
    let mut cx = cx();
    let registry = registry();
    let transport = FixtureTransport::new().with(sym("doc"), doc());
    let mut session = Session::new(transport);

    for pane in ["pane-1", "pane-2"] {
        session
            .open_codec(
                &mut cx,
                &registry,
                sym(pane),
                sym("doc"),
                surface_codec(),
                surface::preset("desktop").unwrap(),
            )
            .unwrap();
    }

    session
        .submit_intent(&mut cx, &registry, &sym("pane-1"), &edit_a_to_9())
        .unwrap();
    let stale = session.submit_intent_at_rendered_revision(
        &mut cx,
        &registry,
        &sym("pane-2"),
        &edit_a_to(doc(), number("8")),
    );
    assert!(stale.is_err(), "stale pane must fail before commit");

    let updates = session.pump(&mut cx, &registry).unwrap();
    assert_eq!(updates.len(), 2, "both panes refresh to the new revision");
    let fresh_value = session.transport_mut().read(&mut cx, &sym("doc")).unwrap();
    session
        .submit_intent_at_rendered_revision(
            &mut cx,
            &registry,
            &sym("pane-2"),
            &edit_a_to(fresh_value, number("8")),
        )
        .unwrap();
}

#[test]
fn surface_caps_project_the_session_scene() {
    let mut cx = cx();
    let registry = registry();
    let value = Expr::List(vec![
        Expr::String("one".into()),
        Expr::String("two".into()),
        Expr::String("three".into()),
        Expr::String("four".into()),
    ]);
    let mut dense = Session::new(FixtureTransport::new().with(sym("doc"), value.clone()));
    let mut glance = Session::new(FixtureTransport::new().with(sym("doc"), value));

    let dense_scene = dense
        .open_codec(
            &mut cx,
            &registry,
            sym("pane"),
            sym("doc"),
            surface_codec(),
            surface::preset("desktop").unwrap(),
        )
        .unwrap();
    let glance_scene = glance
        .open_codec(
            &mut cx,
            &registry,
            sym("pane"),
            sym("doc"),
            surface_codec(),
            surface::preset("watch").unwrap(),
        )
        .unwrap();

    assert!(child_count(&glance_scene) <= child_count(&dense_scene));
    assert_ne!(glance_scene, dense_scene, "glance caps project the scene");
}

#[test]
fn sessions_are_isolated_by_transport_and_subscription_state() {
    let mut cx = cx();
    let registry = registry();
    let mut first = Session::new(FixtureTransport::new().with(sym("doc"), doc()));
    let mut second = Session::new(FixtureTransport::new().with(sym("doc"), doc()));

    for session in [&mut first, &mut second] {
        session
            .open_codec(
                &mut cx,
                &registry,
                sym("pane"),
                sym("doc"),
                surface_codec(),
                surface::preset("desktop").unwrap(),
            )
            .unwrap();
    }

    first
        .submit_intent(&mut cx, &registry, &sym("pane"), &edit_a_to_9())
        .unwrap();
    let first_updates = first.pump(&mut cx, &registry).unwrap();
    let second_updates = second.pump(&mut cx, &registry).unwrap();

    assert_eq!(first_updates.len(), 1);
    assert!(second_updates.is_empty());
    assert_eq!(
        sim_value::access::field(
            &second.transport_mut().read(&mut cx, &sym("doc")).unwrap(),
            "a"
        ),
        Some(&number("1"))
    );
}
