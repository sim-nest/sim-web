//! Tests for the workspace value, layout engine, and persistence.

use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};

use crate::layout::{LayoutOp, apply_layout_op, layout_op_from_intent};
use crate::pane::{pane_dock, pane_id};
use crate::value::{as_int, get, new_workspace, panes};
use crate::{focus, workspace_scene};

fn cx() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::register_core_classes(&mut cx);
    let lisp = sim_codec_lisp::LispCodecLib::new(cx.registry_mut().fresh_codec_id()).unwrap();
    cx.load_lib(&lisp).unwrap();
    let json = sim_codec_json::JsonCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&json).unwrap();
    cx
}

use sim_value::build::keyword as sym;

fn populated_workspace() -> Expr {
    let mut workspace = new_workspace("builder");
    workspace = apply_layout_op(
        &workspace,
        &LayoutOp::Open {
            id: sym("p1"),
            resource: Expr::Symbol(Symbol::qualified("agent", "planner")),
            lens: sym("view:agent-topology"),
            dock: sym("center"),
        },
    )
    .unwrap();
    workspace = apply_layout_op(
        &workspace,
        &LayoutOp::Open {
            id: sym("p2"),
            resource: Expr::String("notes".to_owned()),
            lens: sym("view:default"),
            dock: sym("right"),
        },
    )
    .unwrap();
    workspace
}

#[test]
fn a_workspace_roundtrips_through_two_general_codecs() {
    let mut cx = cx();
    let workspace = populated_workspace();
    for codec in ["lisp", "json"] {
        let restored = sim_test_support::roundtrip(&mut cx, codec, &workspace);
        assert_eq!(
            workspace, restored,
            "workspace must round-trip through codec:{codec}"
        );
        // Panes survive the round-trip as data.
        assert_eq!(panes(&restored).len(), 2);
    }
}

#[test]
fn panes_can_be_created_moved_resized_docked_and_closed() {
    let mut workspace = populated_workspace();
    assert_eq!(panes(&workspace).len(), 2);
    assert_eq!(focus(&workspace), Some(sym("p2")));

    workspace = apply_layout_op(
        &workspace,
        &LayoutOp::Move {
            id: sym("p1"),
            x: 30,
            y: 40,
        },
    )
    .unwrap();
    workspace = apply_layout_op(
        &workspace,
        &LayoutOp::Resize {
            id: sym("p1"),
            w: 800,
            h: 600,
        },
    )
    .unwrap();
    let p1 = pane_named(&workspace, "p1");
    let r = get(&p1, "rect").unwrap();
    assert_eq!(as_int(get(r, "x").unwrap()), Some(30));
    assert_eq!(as_int(get(r, "w").unwrap()), Some(800));

    workspace = apply_layout_op(
        &workspace,
        &LayoutOp::Dock {
            id: sym("p1"),
            dock: sym("left"),
        },
    )
    .unwrap();
    assert_eq!(pane_dock(&pane_named(&workspace, "p1")), Some(sym("left")));
    workspace = apply_layout_op(&workspace, &LayoutOp::Undock { id: sym("p1") }).unwrap();
    assert_eq!(pane_dock(&pane_named(&workspace, "p1")), Some(sym("float")));

    workspace = apply_layout_op(&workspace, &LayoutOp::Close { id: sym("p2") }).unwrap();
    assert_eq!(panes(&workspace).len(), 1);
    assert_eq!(
        focus(&workspace),
        None,
        "closing the focused pane clears focus"
    );
}

#[test]
fn opening_a_duplicate_pane_id_fails() {
    let workspace = populated_workspace();
    let result = apply_layout_op(
        &workspace,
        &LayoutOp::Open {
            id: sym("p1"),
            resource: Expr::Nil,
            lens: sym("view:default"),
            dock: sym("center"),
        },
    );
    assert!(result.is_err());
}

#[test]
fn layout_ops_can_come_from_intents() {
    let open = sim_lib_intent::intent(
        "open",
        sim_lib_intent::Origin::human(1),
        vec![
            ("value", Expr::String("doc".to_owned())),
            ("pane", Expr::Symbol(sym("pX"))),
        ],
    );
    let op = layout_op_from_intent(&open)
        .unwrap()
        .expect("open is a layout op");
    let workspace = apply_layout_op(&new_workspace("household"), &op).unwrap();
    assert_eq!(pane_id(&panes(&workspace)[0]), Some(sym("pX")));

    let dock = sim_lib_intent::intent(
        "invoke",
        sim_lib_intent::Origin::human(1),
        vec![
            ("target", Expr::Symbol(sym("pX"))),
            ("op", Expr::Symbol(sym("dock"))),
            (
                "args",
                crate::value::map(vec![("dock", Expr::Symbol(sym("bottom")))]),
            ),
        ],
    );
    let op = layout_op_from_intent(&dock)
        .unwrap()
        .expect("invoke dock is a layout op");
    let workspace = apply_layout_op(&workspace, &op).unwrap();
    assert_eq!(
        pane_dock(&pane_named(&workspace, "pX")),
        Some(sym("bottom"))
    );

    // A non-layout intent maps to nothing.
    let commit = sim_lib_intent::intent(
        "commit",
        sim_lib_intent::Origin::human(1),
        vec![("pane", Expr::Symbol(sym("pX")))],
    );
    assert!(layout_op_from_intent(&commit).unwrap().is_none());
}

#[test]
fn the_workspace_renders_a_valid_layout_scene() {
    let workspace = populated_workspace();
    let scene = workspace_scene(&workspace);
    sim_lib_scene::validate_scene(&scene).expect("layout scene must be valid");
}

fn pane_named(workspace: &Expr, name: &str) -> Expr {
    panes(workspace)
        .into_iter()
        .find(|pane| pane_id(pane) == Some(sym(name)))
        .expect("pane exists")
}
