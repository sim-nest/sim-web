//! Deterministic cookbook builders for web bridge recipes.

use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_lib_view::{
    LensRegistry, UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, register_universal_default,
};
use sim_value::build::entry;

use crate::{FixtureTransport, Session};

/// Build the fixture-backed session descriptor used by the cookbook recipe.
pub fn session_fixture_demo() -> Expr {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    let registry = registry();
    let mut session = Session::new(FixtureTransport::new().with(resource(), sample_value()));
    let scene = session
        .open(
            &mut cx,
            &registry,
            Symbol::new("pane-1"),
            resource(),
            Symbol::new(UNIVERSAL_VIEW_ID),
            Symbol::new(UNIVERSAL_EDITOR_ID),
        )
        .expect("fixture session opens");
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    Expr::Map(vec![
        entry("transport", Expr::Symbol(Symbol::new("fixture"))),
        entry("resource", Expr::Symbol(resource())),
        entry("pane", Expr::Symbol(Symbol::new("pane-1"))),
        entry("scene", scene),
    ])
}

fn registry() -> LensRegistry {
    let mut registry = LensRegistry::new();
    register_universal_default(&mut registry, false);
    registry
}

fn sample_value() -> Expr {
    Expr::Map(vec![
        entry("title", Expr::String("Fixture-backed session".to_owned())),
        entry("status", Expr::Symbol(Symbol::new("modeled"))),
    ])
}

fn resource() -> Symbol {
    Symbol::qualified("doc", "cookbook")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_fixture_demo_carries_valid_scene() {
        let demo = session_fixture_demo();
        let Expr::Map(entries) = demo else {
            panic!("demo descriptor is a map")
        };
        let scene = entries
            .iter()
            .find_map(|(key, value)| {
                matches!(key, Expr::Symbol(symbol) if &*symbol.name == "scene").then_some(value)
            })
            .expect("scene field exists");
        sim_lib_scene::validate_scene(scene).expect("session scene validates");
    }

    #[test]
    fn canonical_entry_matches_session_cookbook_field_shape() {
        let value = Expr::Symbol(Symbol::new("fixture"));
        assert_eq!(
            entry("transport", value.clone()),
            (Expr::Symbol(Symbol::new("transport")), value)
        );
    }
}
