//! Intent-replay and golden-Scene tests over the fixture session.

use sim_kernel::{Expr, NumberLiteral};
use sim_lib_intent::{Origin, intent};
use sim_lib_view::{
    LensRegistry, UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, register_universal_default,
    universal_scene,
};

use crate::fixture::FixtureTransport;
use crate::session::Session;
use crate::transport::Transport;

use sim_kernel::testing::eager_cx as cx;

fn registry() -> LensRegistry {
    let mut registry = LensRegistry::new();
    register_universal_default(&mut registry, false);
    registry
}

use sim_value::build::keyword as sym;

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

fn edit(field: &str, value: Expr) -> Expr {
    intent(
        "edit-field",
        Origin::human(1),
        vec![
            ("target", doc()),
            (
                "path",
                Expr::List(vec![Expr::Vector(vec![
                    Expr::Symbol(sym("k")),
                    Expr::Symbol(sym(field)),
                ])]),
            ),
            ("value", value),
        ],
    )
}

#[test]
fn replaying_an_intent_stream_reconstructs_value_and_scene() {
    let mut cx = cx();
    let registry = registry();
    let mut session = Session::new(FixtureTransport::new().with(sym("doc"), doc()));
    session
        .open(
            &mut cx,
            &registry,
            sym("pane-1"),
            sym("doc"),
            sym(UNIVERSAL_VIEW_ID),
            sym(UNIVERSAL_EDITOR_ID),
        )
        .unwrap();

    // A recorded Intent stream.
    let stream = [edit("a", number("9")), edit("b", number("8"))];
    for intent in &stream {
        session
            .submit_intent(&mut cx, &registry, &sym("pane-1"), intent)
            .unwrap();
    }
    let updates = session.pump(&mut cx, &registry).unwrap();

    // The resulting value reflects every Intent.
    let value = session.transport_mut().read(&sym("doc")).unwrap();
    assert_eq!(field(&value, "a"), Some(number("9")));
    assert_eq!(field(&value, "b"), Some(number("8")));

    // The resulting Scene equals a fresh render of the final value.
    let final_scene = registry
        .render(&mut cx, &sym(UNIVERSAL_VIEW_ID), &value)
        .unwrap();
    assert_eq!(updates.last().unwrap().scene, final_scene);
}

#[test]
fn the_universal_lens_renders_a_deterministic_golden_scene() {
    let value = doc();
    let first = universal_scene(&value, sim_lib_view::Mode::Builder);
    let second = universal_scene(&value, sim_lib_view::Mode::Builder);
    // Golden property: rendering is a pure function of the value (deterministic).
    assert_eq!(first, second);
    sim_lib_scene::validate_scene(&first).expect("the golden scene is valid");
    // And it is lossless through the portable form (a stable on-disk golden).
    let text = sim_codec::encode_portable(sim_kernel::CodecId(0), &first).unwrap();
    let restored = sim_codec::decode_portable(sim_kernel::CodecId(0), &text).unwrap();
    assert_eq!(first, restored);
}

fn field(map: &Expr, name: &str) -> Option<Expr> {
    let Expr::Map(entries) = map else { return None };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(s) if &*s.name == name).then(|| value.clone())
    })
}
