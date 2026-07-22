use super::*;

use sim_kernel::NumberLiteral;
use sim_lib_intent::{Origin, intent};
use sim_lib_view::surface;

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

/// An `edit-field` Intent setting top-level field `field` to `value`.
fn edit(operator: Origin, field: &str, value: Expr) -> Expr {
    intent(
        "edit-field",
        operator,
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

fn hub_with_surfaces() -> SurfaceHub {
    let mut hub = SurfaceHub::new();
    hub.register_surface(sym("cli"), surface::preset("cli").unwrap());
    hub.register_surface(sym("web"), surface::preset("webui").unwrap());
    hub.register_surface(sym("watch"), surface::preset("watch").unwrap());
    hub
}

#[test]
fn submit_rejects_a_surface_without_the_required_input_capability() {
    let mut hub = SurfaceHub::new();
    let mut caps = surface::preset("webui").unwrap();
    caps.client_id = "no-input".to_owned();
    caps.input = Expr::Map(Vec::new());
    hub.register_surface(sym("no-input"), caps);
    hub.seed(sym("doc"), doc());
    hub.open(&sym("no-input"), sym("pane"), sym("doc")).unwrap();
    let before = hub.canonical(&sym("doc")).cloned();

    let err = hub
        .submit(
            &sym("no-input"),
            &sym("pane"),
            &edit(Origin::human(1), "a", number("9")),
        )
        .unwrap_err();

    assert!(
        err.to_string()
            .contains("does not accept any required input"),
        "unexpected error: {err}"
    );
    assert_eq!(hub.canonical(&sym("doc")).cloned(), before);
    assert!(hub.ledger().is_empty());
}

#[test]
fn an_edit_broadcasts_to_every_surface_viewing_the_resource() {
    let mut hub = hub_with_surfaces();
    hub.seed(sym("doc"), doc());
    let cli_scene = hub.open(&sym("cli"), sym("pane"), sym("doc")).unwrap();
    let web_scene = hub.open(&sym("web"), sym("pane"), sym("doc")).unwrap();

    let broadcasts = hub
        .submit(
            &sym("cli"),
            &sym("pane"),
            &edit(Origin::human(1), "a", number("9")),
        )
        .unwrap();

    // Both surfaces viewing `doc` receive a broadcast.
    assert!(broadcasts.len() >= 2);
    assert!(broadcasts.iter().any(|b| b.surface == sym("cli")));
    assert!(broadcasts.iter().any(|b| b.surface == sym("web")));

    // Each diff reconstructs the surface's new Scene from its prior Scene.
    for broadcast in &broadcasts {
        let prior = if broadcast.surface == sym("cli") {
            &cli_scene
        } else {
            &web_scene
        };
        let rebuilt = sim_lib_scene::apply(prior, &broadcast.diff).unwrap();
        assert_eq!(rebuilt, broadcast.scene);
    }

    let canonical = hub.canonical(&sym("doc")).unwrap();
    assert_eq!(
        sim_value::access::field(canonical, "a").cloned(),
        Some(number("9"))
    );
    assert_eq!(
        sim_value::access::field(canonical, "b").cloned(),
        Some(number("2"))
    );
}

#[test]
fn a_mid_loop_broadcast_error_leaves_canonical_ledger_and_caches_unchanged() {
    let mut hub = hub_with_surfaces();
    hub.seed(sym("doc"), doc());
    let cli_scene = hub.open(&sym("cli"), sym("pane"), sym("doc")).unwrap();
    hub.open(&sym("web"), sym("pane"), sym("doc")).unwrap();

    let canonical_before = hub.canonical(&sym("doc")).cloned();
    let ledger_len_before = hub.ledger().len();

    hub.surfaces.remove(&sym("web"));

    let result = hub.submit(
        &sym("cli"),
        &sym("pane"),
        &edit(Origin::human(1), "a", number("9")),
    );
    assert!(
        result.is_err(),
        "a mid-loop render failure must fail the whole submit"
    );

    assert_eq!(hub.canonical(&sym("doc")).cloned(), canonical_before);
    assert_eq!(hub.ledger().len(), ledger_len_before);
    let cli_last = hub
        .bindings
        .iter()
        .find(|binding| binding.surface == sym("cli") && binding.pane == sym("pane"))
        .map(|binding| binding.last_scene.clone());
    assert_eq!(
        cli_last,
        Some(cli_scene),
        "cli's cached scene must be untouched after the failed submit"
    );
}

#[test]
fn handoff_extends_broadcast_to_the_target_surface() {
    let mut hub = hub_with_surfaces();
    hub.seed(sym("doc"), doc());
    hub.open(&sym("cli"), sym("pane"), sym("doc")).unwrap();
    hub.open(&sym("web"), sym("pane"), sym("doc")).unwrap();

    hub.handoff(&sym("cli"), &sym("watch"), sym("doc"), sym("pane"))
        .unwrap();

    let broadcasts = hub
        .submit(
            &sym("web"),
            &sym("pane"),
            &edit(Origin::human(2), "b", number("7")),
        )
        .unwrap();

    assert!(broadcasts.iter().any(|b| b.surface == sym("cli")));
    assert!(broadcasts.iter().any(|b| b.surface == sym("web")));
    assert!(broadcasts.iter().any(|b| b.surface == sym("watch")));
}

#[test]
fn two_writer_conflict_is_last_write_wins_and_replayable() {
    let mut hub = hub_with_surfaces();
    let seed = doc();
    hub.seed(sym("doc"), seed.clone());
    hub.open(&sym("cli"), sym("pane"), sym("doc")).unwrap();
    hub.open(&sym("web"), sym("pane"), sym("doc")).unwrap();

    hub.submit(
        &sym("cli"),
        &sym("pane"),
        &edit(Origin::human(1), "a", number("10")),
    )
    .unwrap();
    hub.submit(
        &sym("web"),
        &sym("pane"),
        &edit(Origin::agent(2), "a", number("20")),
    )
    .unwrap();

    let canonical = hub.canonical(&sym("doc")).unwrap().clone();
    assert_eq!(
        sim_value::access::field(&canonical, "a").cloned(),
        Some(number("20"))
    );

    let ledger = hub.ledger();
    assert_eq!(ledger.len(), 2);
    assert_eq!(ledger[0].operator, sym("human"));
    assert_eq!(ledger[0].tick, 1);
    assert_eq!(ledger[1].operator, sym("agent"));
    assert_eq!(ledger[1].tick, 2);

    let mut seed_state = BTreeMap::new();
    seed_state.insert(sym("doc"), seed);
    let replayed = replay(ledger, seed_state).expect("ledger rows are all set-value ops");
    assert_eq!(replayed.get(&sym("doc")), Some(&canonical));
}
