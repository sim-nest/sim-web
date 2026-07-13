//! The article lenses: formatted view, source view, edit decoding, and export.
//!
//! Both lenses render one shared [`sim_codec_doc::MarkupDoc`] value. The
//! formatted lens projects semantic blocks to Scene nodes, the source lens shows
//! the deterministic source text for the same markup value, and intent decoding
//! turns validated edit-field gestures into reversible markup edits.

use sim_codec_doc::{Inline, MarkupBlock, MarkupDoc, MarkupEdit};
use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};
use sim_lib_scene::{node, sym};
use sim_lib_view::{Mode, universal_scene};

use crate::doc::{article_from_markup, markup_from_article, title};

/// The formatted article lens id.
pub const ARTICLE_FORMATTED_LENS: &str = "view:doc-article";

/// The source article lens id.
pub const ARTICLE_SOURCE_LENS: &str = "view:doc-source";

/// Attach a cached rendered output (a Scene) to an embed block.
pub fn with_cache(block: Expr, scene: Expr) -> Expr {
    let Expr::Map(mut entries) = block else {
        return block;
    };
    entries.push((Expr::Symbol(Symbol::new("cache")), scene));
    Expr::Map(entries)
}

/// Render the formatted article: outline plus a block canvas.
pub fn article_formatted(doc: &Expr) -> Expr {
    let markup = markup_or_empty(doc);
    let canvas = markup.blocks.iter().map(block_formatted).collect();
    node(
        "stack",
        vec![
            ("role", sym("article")),
            ("dir", sym("row")),
            (
                "children",
                Expr::List(vec![
                    article_outline(doc),
                    node(
                        "stack",
                        vec![
                            ("role", sym("canvas")),
                            ("dir", sym("column")),
                            ("children", Expr::List(canvas)),
                        ],
                    ),
                ]),
            ),
        ],
    )
}

/// Render the source article as deterministic source text.
pub fn article_source(doc: &Expr) -> Expr {
    let markup = markup_or_empty(doc);
    node(
        "stack",
        vec![
            ("role", sym("source")),
            ("dir", sym("column")),
            ("children", Expr::List(vec![text(source_text(&markup))])),
        ],
    )
}

/// The outline: a tree of the article's section headings.
pub fn article_outline(doc: &Expr) -> Expr {
    let markup = markup_or_empty(doc);
    let sections = markup
        .blocks
        .iter()
        .filter_map(|block| match block {
            MarkupBlock::Heading { text, .. } => Some(text_node(inline_text(text))),
            _ => None,
        })
        .collect();
    node(
        "tree",
        vec![
            ("label", Expr::String("outline".to_owned())),
            ("nodes", Expr::List(sections)),
        ],
    )
}

/// Decode a document edit Intent into a reversible markup edit.
///
/// # Errors
///
/// Returns an error when the Intent is not `intent/edit-field`, its path does
/// not address a block, or its replacement value is not a markup block.
pub fn markup_edit_from_intent(doc: &Expr, intent: &Expr) -> Result<MarkupEdit> {
    let kind = sim_lib_intent::intent_kind_of(intent)
        .ok_or_else(|| Error::Eval("intent is missing kind".to_owned()))?;
    if kind != sim_lib_intent::intent_kind("edit-field") {
        return Err(Error::Eval("expected intent/edit-field".to_owned()));
    }
    let markup = markup_from_article(doc)?;
    let path = sim_lib_intent::field(intent, "path")
        .ok_or_else(|| Error::Eval("edit intent is missing path".to_owned()))?;
    let index = block_index_from_path(path)?;
    let old = markup
        .blocks
        .get(index)
        .cloned()
        .ok_or_else(|| Error::Eval("edit intent block index is out of range".to_owned()))?;
    let value = sim_lib_intent::field(intent, "value")
        .ok_or_else(|| Error::Eval("edit intent is missing value".to_owned()))?;
    let new = MarkupBlock::from_expr(value)?;
    Ok(MarkupEdit::ReplaceBlock { index, old, new })
}

/// The stable intermediate document value used for export.
pub fn export_intermediate(doc: &Expr) -> Expr {
    article_from_markup(&markup_or_empty(doc))
}

/// Export the document through the shared markup source writer.
pub fn export_markdown(doc: &Expr) -> String {
    source_text(&markup_or_empty(doc))
}

fn markup_or_empty(doc: &Expr) -> MarkupDoc {
    markup_from_article(doc).unwrap_or_else(|_| MarkupDoc {
        title: title(doc).or_else(|| Some("untitled".to_owned())),
        blocks: Vec::new(),
        attrs: Default::default(),
        source: None,
    })
}

fn source_text(markup: &MarkupDoc) -> String {
    let body = markup.to_source_text();
    let Some(title) = &markup.title else {
        return body;
    };
    if title_is_first_heading(markup, title) {
        return body;
    }
    if body.is_empty() {
        format!("# {title}\n")
    } else {
        format!("# {title}\n\n{body}")
    }
}

fn title_is_first_heading(markup: &MarkupDoc, title: &str) -> bool {
    matches!(
        markup.blocks.first(),
        Some(MarkupBlock::Heading { level: 1, text, .. }) if inline_text(text) == title
    )
}

fn block_formatted(block: &MarkupBlock) -> Expr {
    match block {
        MarkupBlock::Heading { text, .. } => node(
            "text",
            vec![
                ("role", sym("heading")),
                ("text", Expr::String(inline_text(text))),
            ],
        ),
        MarkupBlock::Paragraph { content, .. } => {
            node("text", vec![("text", Expr::String(inline_text(content)))])
        }
        MarkupBlock::CodeBlock { code, .. } => boxed_text("code", code),
        MarkupBlock::MathBlock { source, .. } => boxed_text("equation", &source.text),
        MarkupBlock::Quote { blocks, .. } => node(
            "box",
            vec![
                ("role", sym("quote")),
                (
                    "children",
                    Expr::List(blocks.iter().map(block_formatted).collect()),
                ),
            ],
        ),
        MarkupBlock::List { items, .. } => node(
            "stack",
            vec![
                ("role", sym("list")),
                ("dir", sym("column")),
                (
                    "children",
                    Expr::List(
                        items
                            .iter()
                            .map(|item| {
                                node(
                                    "stack",
                                    vec![
                                        ("role", sym("list-item")),
                                        ("dir", sym("column")),
                                        (
                                            "children",
                                            Expr::List(item.iter().map(block_formatted).collect()),
                                        ),
                                    ],
                                )
                            })
                            .collect(),
                    ),
                ),
            ],
        ),
        MarkupBlock::Table { header, rows, .. } => table_scene(header, rows),
        MarkupBlock::Figure { src, caption, .. } => node(
            "box",
            vec![
                ("role", sym("figure")),
                ("src", Expr::String(src.clone())),
                (
                    "children",
                    Expr::List(vec![text_node(inline_text(caption))]),
                ),
            ],
        ),
        MarkupBlock::Raw { backend, text, .. } if backend.as_str() == "view-doc/embed" => {
            embed_scene(text)
        }
        MarkupBlock::Raw { backend, text, .. } if backend.as_str() == "view-doc/citation" => node(
            "text",
            vec![
                ("role", sym("citation")),
                ("text", Expr::String(text.clone())),
            ],
        ),
        MarkupBlock::Raw { text, .. } => text_node(text.clone()),
    }
}

fn boxed_text(role: &str, content: &str) -> Expr {
    node(
        "box",
        vec![
            ("role", sym(role)),
            ("children", Expr::List(vec![text_node(content.to_owned())])),
        ],
    )
}

fn embed_scene(text: &str) -> Expr {
    let inner = universal_scene(&Expr::String(text.to_owned()), Mode::Builder);
    node(
        "embed",
        vec![
            ("lens", Expr::Symbol(Symbol::new("view:default"))),
            ("scene", inner),
        ],
    )
}

fn table_scene(header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) -> Expr {
    let rows = std::iter::once(header)
        .chain(rows.iter().map(Vec::as_slice))
        .map(|row| {
            Expr::List(
                row.iter()
                    .map(|cell| Expr::String(inline_text(cell)))
                    .collect(),
            )
        })
        .collect();
    node("table", vec![("rows", Expr::List(rows))])
}

fn block_index_from_path(path: &Expr) -> Result<usize> {
    let segments = path_segments(path)?;
    if segments.len() != 2 || segment_name(segments[0]).as_deref() != Some("blocks") {
        return Err(Error::Eval(
            "edit intent path must address blocks/<index>".to_owned(),
        ));
    }
    segment_index(segments[1])
}

fn path_segments(path: &Expr) -> Result<Vec<&Expr>> {
    match path {
        Expr::List(items) if items.len() == 1 => match &items[0] {
            Expr::Vector(items) => Ok(items.iter().collect()),
            _ => Ok(items.iter().collect()),
        },
        Expr::List(items) => Ok(items.iter().collect()),
        _ => Err(Error::Eval("edit intent path must be a list".to_owned())),
    }
}

fn segment_name(segment: &Expr) -> Option<String> {
    match segment {
        Expr::String(text) => Some(text.clone()),
        Expr::Symbol(symbol) => Some(symbol.name.to_string()),
        _ => None,
    }
}

fn segment_index(segment: &Expr) -> Result<usize> {
    match segment {
        Expr::Number(NumberLiteral { canonical, .. }) => canonical
            .parse()
            .map_err(|_| Error::Eval("block index must be an integer".to_owned())),
        Expr::String(text) => text
            .parse()
            .map_err(|_| Error::Eval("block index must be an integer".to_owned())),
        _ => Err(Error::Eval("block index must be a number".to_owned())),
    }
}

fn inline_text(items: &[Inline]) -> String {
    let mut out = String::new();
    for item in items {
        match item {
            Inline::Text(text) | Inline::Code(text) => out.push_str(text),
            Inline::Emph(children) | Inline::Strong(children) => {
                out.push_str(&inline_text(children))
            }
            Inline::Link { label, .. } => out.push_str(&inline_text(label)),
            Inline::Math(source) => out.push_str(&source.text),
            Inline::Raw { text, .. } => out.push_str(text),
        }
    }
    out
}

fn text(content: String) -> Expr {
    text_node(content)
}

fn text_node(content: String) -> Expr {
    node("text", vec![("text", Expr::String(content))])
}
