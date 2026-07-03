//! Tests for the universal default view and editor.

use sim_kernel::{Expr, NumberLiteral, Symbol};
use sim_lib_intent::{Origin, intent};

use crate::contract::{LensKind, View};
use crate::dispatch::{DispatchContext, DispatchReason, LensRegistry};
use crate::universal::{UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, register_universal_default};
use crate::universal_editor::{EDIT_MODES, UniversalEditor, render_draft};
use crate::universal_view::UniversalView;
use crate::{Draft, Editor};

use sim_kernel::testing::eager_cx as cx;

use sim_value::build::sym;

fn number(value: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::new("i64"),
        canonical: value.to_owned(),
    })
}

fn sample_map() -> Expr {
    Expr::Map(vec![
        (sym("a"), number("1")),
        (sym("b"), number("2")),
        (
            sym("nested"),
            Expr::List(vec![Expr::String("x".to_owned()), Expr::Bool(true)]),
        ),
    ])
}

#[test]
fn universal_view_renders_a_valid_four_region_scene() {
    let mut cx = cx();
    for value in [
        Expr::Nil,
        number("42"),
        Expr::String("hello".to_owned()),
        sample_map(),
        Expr::List(vec![Expr::Nil, sym("z")]),
    ] {
        let scene = UniversalView.encode(&mut cx, &value).unwrap();
        sim_lib_scene::validate_scene(&scene)
            .unwrap_or_else(|err| panic!("scene invalid for {value:?}: {err}"));
        // The root stack has exactly four regions.
        let children = region_children(&scene);
        assert_eq!(children, 4, "value {value:?} must open four regions");
    }
}

fn region_children(scene: &Expr) -> usize {
    let Expr::Map(entries) = scene else { return 0 };
    for (key, value) in entries {
        if matches!(key, Expr::Symbol(s) if &*s.name == "children")
            && let Expr::List(items) = value
        {
            return items.len();
        }
    }
    0
}

#[test]
fn any_value_dispatches_to_the_universal_default() {
    let mut cx = cx();
    let mut registry = LensRegistry::new();
    register_universal_default(&mut registry, false);
    let grant = |_: &sim_kernel::CapabilityName| true;
    let ctx = DispatchContext::permissive(&grant);
    let outcome = registry.dispatch_view(&mut cx, &Expr::Nil, &ctx).unwrap();
    assert_eq!(outcome.lens_id, Symbol::new(UNIVERSAL_VIEW_ID));
    assert_eq!(outcome.reason, DispatchReason::UniversalDefault);
}

#[test]
fn editing_a_field_commits_and_preserves_siblings() {
    let mut cx = cx();
    let editor = UniversalEditor::writable();
    let value = sample_map();
    // edit-field path [k a] := 9
    let edit = intent(
        "edit-field",
        Origin::human(1),
        vec![
            ("target", value.clone()),
            (
                "path",
                Expr::List(vec![Expr::Vector(vec![sym("k"), sym("a")])]),
            ),
            ("value", number("9")),
        ],
    );
    let draft = editor.decode(&mut cx, &value, &edit).unwrap();
    assert!(draft.committable, "a valid edit must be committable");
    // The proposed value updated `a` and preserved `b` and `nested`.
    let Expr::Map(entries) = &draft.proposed else {
        panic!("proposed must be a map")
    };
    assert_eq!(entries.len(), 3, "unknown fields preserved");
    let a = entries
        .iter()
        .find(|(k, _)| matches!(k, Expr::Symbol(s) if &*s.name == "a"))
        .map(|(_, v)| v);
    assert_eq!(a, Some(&number("9")));

    let op = editor.commit(&mut cx, &draft).unwrap();
    let Expr::Map(form) = &op.form else {
        panic!("operation form must be a map")
    };
    assert!(
        form.iter()
            .any(|(k, _)| matches!(k, Expr::Symbol(s) if &*s.name == "op"))
    );
}

#[test]
fn an_unknown_path_is_a_field_anchored_diagnostic_not_a_commit() {
    let mut cx = cx();
    let editor = UniversalEditor::writable();
    let value = sample_map();
    let edit = intent(
        "edit-field",
        Origin::human(1),
        vec![
            ("target", value.clone()),
            (
                "path",
                Expr::List(vec![
                    Expr::Vector(vec![sym("k"), sym("missing")]),
                    Expr::Vector(vec![sym("k"), sym("deep")]),
                ]),
            ),
            ("value", number("9")),
        ],
    );
    let draft = editor.decode(&mut cx, &value, &edit).unwrap();
    assert!(!draft.committable, "an unknown nested path must not commit");
    assert!(!draft.diagnostics.is_empty(), "must carry a diagnostic");
    assert!(
        editor.commit(&mut cx, &draft).is_err(),
        "commit must fail closed"
    );
}

#[test]
fn readonly_editor_cannot_commit() {
    let mut cx = cx();
    let editor = UniversalEditor::readonly();
    let value = sample_map();
    let edit = intent(
        "edit-field",
        Origin::human(1),
        vec![
            ("target", value.clone()),
            (
                "path",
                Expr::List(vec![Expr::Vector(vec![sym("k"), sym("a")])]),
            ),
            ("value", number("9")),
        ],
    );
    let draft = editor.decode(&mut cx, &value, &edit).unwrap();
    assert!(!draft.committable, "readonly edits never commit");
    assert!(editor.commit(&mut cx, &draft).is_err());
}

#[test]
fn cancel_reverts_to_the_base() {
    let mut cx = cx();
    let editor = UniversalEditor::writable();
    let value = sample_map();
    let cancel = intent("cancel", Origin::human(1), vec![("pane", sym("p"))]);
    let draft = editor.decode(&mut cx, &value, &cancel).unwrap();
    assert_eq!(draft.proposed, draft.base, "cancel discards pending edits");
}

#[test]
fn every_advertised_edit_mode_renders_a_valid_scene_from_one_draft() {
    let draft = Draft::clean(sample_map(), sample_map());
    assert_eq!(
        EDIT_MODES,
        ["text", "raw"],
        "only the real modes are advertised"
    );
    for mode in EDIT_MODES {
        let scene = render_draft(&draft, mode).unwrap();
        sim_lib_scene::validate_scene(&scene)
            .unwrap_or_else(|err| panic!("mode {mode} scene invalid: {err}"));
    }
}

/// Recursively collect the `path` attribute of every `field` node in a scene.
fn field_paths(value: &Expr, out: &mut Vec<Expr>) {
    match value {
        Expr::Map(entries) => {
            let is_field = entries.iter().any(|(k, v)| {
                matches!(k, Expr::Symbol(s) if &*s.name == "kind")
                    && matches!(v, Expr::Symbol(s) if &*s.name == "field")
            });
            if is_field
                && let Some(path) = entries.iter().find_map(|(k, v)| {
                    matches!(k, Expr::Symbol(s) if &*s.name == "path").then(|| v.clone())
                })
            {
                out.push(path);
            }
            for (_, v) in entries {
                field_paths(v, out);
            }
        }
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => {
            for item in items {
                field_paths(item, out);
            }
        }
        _ => {}
    }
}

#[test]
fn canonical_text_fields_scope_to_leaf_paths_and_preserve_siblings() {
    let mut cx = cx();
    let value = sample_map();
    let scene = UniversalView.encode(&mut cx, &value).unwrap();

    let mut paths = Vec::new();
    field_paths(&scene, &mut paths);
    assert!(!paths.is_empty(), "scalar leaves must be editable fields");
    let root = Expr::List(vec![]);
    for path in &paths {
        assert_ne!(
            *path, root,
            "no canonical-text field may bind to the root path (that clobbers the whole value)"
        );
    }

    // Driving an edit-field through one scoped path preserves every sibling key.
    let path = paths[0].clone();
    let edit = intent(
        "edit-field",
        Origin::human(1),
        vec![
            ("target", value.clone()),
            ("path", path),
            ("value", number("99")),
        ],
    );
    let editor = UniversalEditor::writable();
    let draft = editor.decode(&mut cx, &value, &edit).unwrap();
    assert!(draft.committable, "a scoped leaf edit commits");
    let Expr::Map(entries) = &draft.proposed else {
        panic!("proposed must stay a map")
    };
    assert_eq!(
        entries.len(),
        3,
        "the scoped edit preserved every sibling key"
    );
}

#[test]
fn universal_default_lens_ids_are_distinct_kinds() {
    let mut registry = LensRegistry::new();
    register_universal_default(&mut registry, false);
    let view = registry.get(&Symbol::new(UNIVERSAL_VIEW_ID)).unwrap();
    let editor = registry.get(&Symbol::new(UNIVERSAL_EDITOR_ID)).unwrap();
    assert_eq!(view.meta.kind, LensKind::View);
    assert_eq!(editor.meta.kind, LensKind::Editor);
}
