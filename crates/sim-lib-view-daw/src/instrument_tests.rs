//! Tests for instrument-specific editor lenses.

use sim_kernel::Expr;
use sim_lib_music_synth::{
    DX7_EDITOR_FIXTURE_NAMES, INSTRUMENT_EDITOR_ROUTE_NAMES, INSTRUMENT_EDITOR_VIEW_IDS,
    PS3300_EDITOR_FIXTURE_NAMES, SYSTEM55_EDITOR_FIXTURE_NAMES, SYSTEM700_EDITOR_FIXTURE_NAMES,
    instrument_editor_descriptors,
};

use crate::dx7::{
    DX7_EDITOR_ROUTE, DX7_EDITOR_VIEW, dx7_editor_fixture_names, dx7_editor_snapshot,
};
use crate::ps3300::{
    PS3300_EDITOR_ROUTE, PS3300_EDITOR_VIEW, ps3300_editor_fixture_names, ps3300_editor_snapshot,
};
use crate::system55::{
    SYSTEM55_EDITOR_ROUTE, SYSTEM55_EDITOR_VIEW, system55_editor_fixture_names,
    system55_editor_snapshot,
};
use crate::system700::{
    SYSTEM700_EDITOR_ROUTE, SYSTEM700_EDITOR_VIEW, system700_editor_fixture_names,
    system700_editor_snapshot,
};

use sim_value::build::sym;

#[test]
fn instrument_editor_snapshots_cover_routes_views_and_fixtures() {
    assert_eq!(
        dx7_editor_fixture_names(),
        DX7_EDITOR_FIXTURE_NAMES.as_slice()
    );
    assert_eq!(
        system700_editor_fixture_names(),
        SYSTEM700_EDITOR_FIXTURE_NAMES.as_slice()
    );
    assert_eq!(
        system55_editor_fixture_names(),
        SYSTEM55_EDITOR_FIXTURE_NAMES.as_slice()
    );
    assert_eq!(
        ps3300_editor_fixture_names(),
        PS3300_EDITOR_FIXTURE_NAMES.as_slice()
    );

    for descriptor in instrument_editor_descriptors() {
        assert!(INSTRUMENT_EDITOR_ROUTE_NAMES.contains(&descriptor.route_name));
        assert!(INSTRUMENT_EDITOR_VIEW_IDS.contains(&descriptor.view_id));
    }

    assert_editor_fixtures(
        dx7_editor_fixture_names(),
        DX7_EDITOR_ROUTE,
        DX7_EDITOR_VIEW,
        dx7_editor_snapshot,
    );
    assert_editor_fixtures(
        system700_editor_fixture_names(),
        SYSTEM700_EDITOR_ROUTE,
        SYSTEM700_EDITOR_VIEW,
        system700_editor_snapshot,
    );
    assert_editor_fixtures(
        system55_editor_fixture_names(),
        SYSTEM55_EDITOR_ROUTE,
        SYSTEM55_EDITOR_VIEW,
        system55_editor_snapshot,
    );
    assert_editor_fixtures(
        ps3300_editor_fixture_names(),
        PS3300_EDITOR_ROUTE,
        PS3300_EDITOR_VIEW,
        ps3300_editor_snapshot,
    );
}

#[test]
fn dx7_editor_exposes_import_algorithm_ops_egs_compare_and_traces() {
    let default = dx7_editor_snapshot(DX7_EDITOR_FIXTURE_NAMES[0]).unwrap();
    assert!(contains_role(&default, "dx7-editor"));
    assert!(contains_role(&default, "sysex-import"));
    assert!(contains_role(&default, "algorithm-editor"));
    assert!(contains_role(&default, "operator-grid"));
    assert!(contains_role(&default, "eg-editor"));
    assert!(contains_role(&default, "pitch-editor"));
    assert!(contains_role(&default, "lfo-editor"));
    assert!(contains_role(&default, "dx7-compare-view"));
    assert!(contains_role(&default, "dx7-trace-view"));

    let invalid = dx7_editor_snapshot(DX7_EDITOR_FIXTURE_NAMES[2]).unwrap();
    assert!(contains_role(&invalid, "instrument-editor-validation"));

    let all_algorithms = dx7_editor_snapshot(DX7_EDITOR_FIXTURE_NAMES[3]).unwrap();
    assert!(contains_role(&all_algorithms, "all-algorithm-compare"));
}

#[test]
fn modular_and_poly_instrument_editors_expose_panel_surfaces() {
    let system700 = system700_editor_snapshot(SYSTEM700_EDITOR_FIXTURE_NAMES[3]).unwrap();
    assert!(contains_role(&system700, "system700-panel-editor"));
    assert!(contains_role(&system700, "modular-patch-panel"));
    assert!(contains_role(&system700, "system700-panel-graph"));
    assert!(contains_role(&system700, "cord-editor"));
    assert!(contains_role(&system700, "system700-trace-view"));

    let system55 = system55_editor_snapshot(SYSTEM55_EDITOR_FIXTURE_NAMES[3]).unwrap();
    assert!(contains_role(&system55, "system55-cabinet-editor"));
    assert!(contains_role(&system55, "cabinet-row"));
    assert!(contains_role(&system55, "system55-cabinet-graph"));
    assert!(contains_role(&system55, "fixed-filter-bank-editor"));
    assert!(contains_role(&system55, "s-trigger-panel"));

    let ps3300 = ps3300_editor_snapshot(PS3300_EDITOR_FIXTURE_NAMES[3]).unwrap();
    assert!(contains_role(&ps3300, "ps3300-panel-editor"));
    assert!(contains_role(&ps3300, "section-editor"));
    assert!(contains_role(&ps3300, "poly-patch-panel"));
    assert!(contains_role(&ps3300, "pin-matrix-editor"));
    assert!(contains_role(&ps3300, "resonator-panel"));
}

fn assert_editor_fixtures(
    fixture_names: &[&str],
    route: &str,
    view: &str,
    snapshot: fn(&str) -> Option<Expr>,
) {
    for fixture_name in fixture_names {
        let scene = snapshot(fixture_name).expect("instrument editor fixture");
        sim_lib_scene::validate_scene(&scene).expect("instrument editor scene is valid");
        assert_eq!(scene, snapshot(fixture_name).unwrap());
        assert_eq!(field(&scene, "lens"), Some(sym(view)));
        assert_eq!(field(&scene, "route"), Some(sym(route)));
        assert!(
            matches!(field(&scene, "fixture-names"), Some(Expr::List(items)) if items.len() == fixture_names.len())
        );
    }
}

fn field(map: &Expr, name: &str) -> Option<Expr> {
    let Expr::Map(entries) = map else { return None };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(s) if &*s.name == name).then(|| value.clone())
    })
}

fn contains_role(expr: &Expr, role: &str) -> bool {
    field(expr, "role") == Some(sym(role))
        || expr_children(expr)
            .iter()
            .any(|child| contains_role(child, role))
}

fn expr_children(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) | Expr::Block(items) => {
            items.iter().collect()
        }
        Expr::Map(entries) => entries
            .iter()
            .flat_map(|(key, value)| [key, value])
            .collect(),
        Expr::Call { operator, args } => std::iter::once(operator.as_ref()).chain(args).collect(),
        Expr::Infix { left, right, .. } => vec![left, right],
        Expr::Prefix { arg, .. } | Expr::Postfix { arg, .. } => vec![arg],
        Expr::Quote { expr, .. } => vec![expr],
        Expr::Annotated { expr, annotations } => std::iter::once(expr.as_ref())
            .chain(annotations.iter().map(|(_, value)| value))
            .collect(),
        Expr::Extension { payload, .. } => vec![payload],
        _ => Vec::new(),
    }
}
