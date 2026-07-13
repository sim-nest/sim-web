//! Tests for the document model, article lenses, and round-trip.

use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_value::build::{sym, uint};

use crate::doc::{
    article, article_from_markup, block_kind, blocks, citation, embed_block, equation, figure,
    markup_from_article, prose, section, table,
};
use crate::lens::{
    article_formatted, article_outline, article_source, export_intermediate, export_markdown,
    markup_edit_from_intent,
};

fn cx() -> Cx {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::register_core_classes(&mut cx);
    let lisp = sim_codec_lisp::LispCodecLib::new(cx.registry_mut().fresh_codec_id()).unwrap();
    cx.load_lib(&lisp).unwrap();
    let json = sim_codec_json::JsonCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&json).unwrap();
    cx
}

fn sample_article() -> Expr {
    article(
        "On Topologies",
        vec![
            section("Introduction"),
            prose("We study agent topologies."),
            equation("E = mc^2"),
            figure("A flow diagram", "flow.svg"),
            table(vec![
                vec![Expr::String("a".to_owned()), Expr::String("b".to_owned())],
                vec![Expr::String("1".to_owned()), Expr::String("2".to_owned())],
            ]),
            citation("knuth1984", "Knuth, Literate Programming, 1984"),
            // An embedded live block: a runtime value rendered by its own lens.
            embed_block(
                Expr::Map(vec![(
                    Expr::Symbol(Symbol::new("result")),
                    Expr::String("42".to_owned()),
                )]),
                "view:default",
            ),
        ],
    )
}

#[test]
fn an_article_with_equation_and_embed_roundtrips_as_sim_data() {
    let mut cx = cx();
    let doc = sample_article();
    // It has at least one equation block and one embedded live block.
    let kinds: Vec<String> = blocks(&doc)
        .iter()
        .filter_map(|b| block_kind(b).map(|k| k.name.to_string()))
        .collect();
    assert!(kinds.contains(&"equation".to_owned()));
    assert!(kinds.contains(&"embed".to_owned()));

    for codec in ["lisp", "json"] {
        let restored = sim_test_support::roundtrip(&mut cx, codec, &doc);
        assert_eq!(
            doc, restored,
            "the article round-trips through codec:{codec}"
        );
    }
}

#[test]
fn the_formatted_and_source_lenses_render_the_same_document() {
    let doc = sample_article();
    let markup = markup_from_article(&doc).expect("sample article is markup-backed");
    let formatted = article_formatted(&doc);
    let source = article_source(&doc);
    assert_eq!(article_from_markup(&markup), export_intermediate(&doc));
    sim_lib_scene::validate_scene(&formatted).expect("formatted view is a valid scene");
    sim_lib_scene::validate_scene(&source).expect("source view is a valid scene");
    sim_lib_scene::validate_scene(&article_outline(&doc)).expect("outline is a valid scene");
}

#[test]
fn edit_intent_yields_markup_replace_block() {
    let doc = sample_article();
    let replacement = sim_codec_doc::MarkupBlock::Paragraph {
        content: vec![sim_codec_doc::Inline::Text("Updated prose.".to_owned())],
        span: None,
    };
    let intent = sim_lib_intent::intent(
        "edit-field",
        sim_lib_intent::Origin::human(1),
        vec![
            ("target", sym("article")),
            (
                "path",
                Expr::List(vec![Expr::String("blocks".to_owned()), uint(1)]),
            ),
            ("value", replacement.as_expr()),
        ],
    );

    let edit = markup_edit_from_intent(&doc, &intent).unwrap();

    let sim_codec_doc::MarkupEdit::ReplaceBlock { index, old, new } = edit else {
        panic!("expected ReplaceBlock edit");
    };
    assert_eq!(index, 1);
    assert!(
        matches!(old, sim_codec_doc::MarkupBlock::Paragraph { content, .. } if inline_text(&content) == "We study agent topologies.")
    );
    assert_eq!(new, replacement);
}

#[test]
fn an_embedded_block_renders_through_scene_embed() {
    let doc = sample_article();
    let formatted = article_formatted(&doc);
    // Somewhere in the formatted scene there is a scene/embed node.
    assert!(
        contains_embed(&formatted),
        "the embedded block renders as scene/embed"
    );
}

#[test]
fn export_runs_over_a_stable_intermediate() {
    let doc = sample_article();
    let intermediate = export_intermediate(&doc);
    assert_eq!(
        intermediate,
        export_intermediate(&intermediate),
        "export IR is stable"
    );
    let markdown = export_markdown(&doc);
    assert!(markdown.contains("# On Topologies"));
    assert!(markdown.contains("E = mc^2"));
}

fn contains_embed(scene: &Expr) -> bool {
    if sim_lib_scene::node_kind(scene).map(|k| k.name.to_string()) == Some("embed".to_owned()) {
        return true;
    }
    match scene {
        Expr::Map(entries) => entries.iter().any(|(_, v)| contains_embed(v)),
        Expr::List(items) | Expr::Vector(items) => items.iter().any(contains_embed),
        _ => false,
    }
}

fn inline_text(items: &[sim_codec_doc::Inline]) -> String {
    items
        .iter()
        .map(|item| match item {
            sim_codec_doc::Inline::Text(text) => text.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}
