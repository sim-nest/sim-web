//! Deterministic cookbook builders for the local view host facade.

use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::BrowserHost;

/// Build a modeled render/edit/update descriptor for the view host loop.
pub fn host_loop_demo() -> Expr {
    let mut host = BrowserHost::new(sample_value()).expect("host demo renders");
    let initial_scene = host.scene().clone();
    let update = host
        .edit_field(
            path_key("title"),
            Expr::String("Edited from the host loop".to_owned()),
        )
        .expect("host demo edit applies")
        .expect("host demo mutates");
    Expr::Map(vec![
        field("initial-scene", initial_scene),
        field("updated-scene", update.scene),
        field("diff", update.diff),
        field("value", host.value().clone()),
    ])
}

fn sample_value() -> Expr {
    Expr::Map(vec![
        field("title", Expr::String("Host loop sample".to_owned())),
        field("count", number("1")),
    ])
}

fn path_key(key: &str) -> Expr {
    Expr::List(vec![Expr::Vector(vec![
        Expr::Symbol(Symbol::new("k")),
        Expr::Symbol(Symbol::new(key)),
    ])])
}

fn number(value: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::new("i64"),
        canonical: value.to_owned(),
    })
}

fn field(key: &str, value: Expr) -> (Expr, Expr) {
    (Expr::Symbol(Symbol::new(key)), value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_loop_demo_carries_valid_scenes_and_diff() {
        let demo = host_loop_demo();
        let Expr::Map(entries) = demo else {
            panic!("demo descriptor is a map")
        };
        let initial = lookup(&entries, "initial-scene");
        let updated = lookup(&entries, "updated-scene");
        let diff = lookup(&entries, "diff");
        sim_lib_scene::validate_scene(initial).expect("initial scene validates");
        sim_lib_scene::validate_scene(updated).expect("updated scene validates");
        assert_eq!(sim_lib_scene::apply(initial, diff).unwrap(), *updated);
    }

    fn lookup<'a>(entries: &'a [(Expr, Expr)], key: &str) -> &'a Expr {
        entries
            .iter()
            .find_map(|(entry_key, value)| {
                matches!(entry_key, Expr::Symbol(symbol) if &*symbol.name == key).then_some(value)
            })
            .expect("field exists")
    }
}
