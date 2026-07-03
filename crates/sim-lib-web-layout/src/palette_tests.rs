//! Tests for the command palette and card-to-lens integration.

use sim_kernel::{Expr, Symbol};
use sim_lib_intent::{intent_kind_of, validate_intent};

use crate::layout::{LayoutOp, apply_layout_op};
use crate::palette::{EntryKind, Palette, PaletteEntry, open_card};
use crate::value::new_workspace;

use sim_value::build::sym;

fn populated_palette() -> Palette {
    let mut palette = Palette::new();
    palette.add(PaletteEntry::new(
        "view:graph",
        "Graph lens",
        EntryKind::Lens,
        sym("view:graph"),
    ));
    palette.add(PaletteEntry::new(
        "planner",
        "planner agent",
        EntryKind::Resource,
        Expr::Symbol(Symbol::qualified("agent", "planner")),
    ));
    palette.add(PaletteEntry::new(
        "save",
        "Save workspace",
        EntryKind::Command,
        sym("workspace"),
    ));
    palette.add(PaletteEntry::new(
        "card:topology",
        "Help: topology graph",
        EntryKind::Card,
        sym("topology"),
    ));
    palette
}

#[test]
fn the_palette_finds_entries_across_kinds() {
    let palette = populated_palette();
    let graph_hits = palette.search("graph");
    assert!(
        graph_hits.iter().any(|e| e.kind == EntryKind::Lens),
        "searching 'graph' finds the graph lens"
    );
    // Substring matches rank above subsequence matches.
    assert_eq!(palette.search("save")[0].id, Symbol::new("save"));
    // Empty query returns everything.
    assert_eq!(palette.search("").len(), 4);
    // A miss returns nothing.
    assert!(palette.search("zzzz").is_empty());
}

#[test]
fn opening_each_entry_kind_produces_the_right_intent() {
    let palette = populated_palette();
    let by_id = |id: &str| {
        palette
            .entries()
            .iter()
            .find(|e| e.id == Symbol::new(id))
            .unwrap()
    };

    let lens = palette.open_entry(by_id("view:graph"), "pane-1");
    assert_eq!(kind_name(&lens), "set-lens");
    validate_intent(&lens).unwrap();

    let resource = palette.open_entry(by_id("planner"), "pane-1");
    assert_eq!(kind_name(&resource), "open");
    validate_intent(&resource).unwrap();

    let command = palette.open_entry(by_id("save"), "pane-1");
    assert_eq!(kind_name(&command), "invoke");
    validate_intent(&command).unwrap();
}

#[test]
fn a_card_opens_into_the_best_lens_for_its_target() {
    // A browse/help card carrying a target.
    let card = Expr::Map(vec![
        (
            sym("class"),
            Expr::Symbol(Symbol::qualified("core", "Card")),
        ),
        (
            sym("target"),
            Expr::Symbol(Symbol::qualified("agent", "planner")),
        ),
    ]);
    let open = open_card(&card, "pane-1").expect("a card with a target opens");
    assert_eq!(kind_name(&open), "open");
    validate_intent(&open).unwrap();
    // The opened value is the card's target, not the card itself; the dispatcher
    // then picks the best lens for that target.
    assert_eq!(
        crate::palette::card_target(&card),
        Some(Expr::Symbol(Symbol::qualified("agent", "planner")))
    );
}

#[test]
fn the_palette_renders_a_valid_scene_and_indexes_open_resources() {
    let mut palette = Palette::new();
    let mut workspace = new_workspace("builder");
    workspace = apply_layout_op(
        &workspace,
        &LayoutOp::Open {
            id: Symbol::new("p1"),
            resource: Expr::String("notes".to_owned()),
            lens: Symbol::new("view:default"),
            dock: Symbol::new("center"),
        },
    )
    .unwrap();
    palette.add_workspace_resources(&workspace);
    assert_eq!(
        palette.entries().len(),
        1,
        "open panes become resource entries"
    );

    let scene = palette.scene("");
    sim_lib_scene::validate_scene(&scene).expect("the palette is a valid scene");
}

fn kind_name(intent: &Expr) -> String {
    intent_kind_of(intent)
        .map(|s| s.name.to_string())
        .unwrap_or_default()
}
