//! The document value adapters.
//!
//! The canonical document value is [`sim_codec_doc::MarkupDoc`]. The helpers in
//! this module keep the article-shaped constructor names as compatibility
//! wrappers while the lenses read and write the shared markup frontend.

use sim_codec_doc::{BackendId, Inline, MarkupBlock, MarkupDoc, MathSource};
use sim_kernel::{Error, Expr, Result, Symbol};

/// The article class symbol carried in the document's `class` field.
pub const ARTICLE_CLASS: &str = "doc/Article";

/// The key that tags a block with its kind.
pub const BLOCK_KEY: &str = "block";

pub use sim_value::access::field;

fn key(name: &str) -> Expr {
    Expr::Symbol(Symbol::new(name))
}

fn map(entries: Vec<(&str, Expr)>) -> Expr {
    Expr::Map(entries.into_iter().map(|(k, v)| (key(k), v)).collect())
}

/// Build a canonical article document from a markup document.
pub fn article_from_markup(markup: &MarkupDoc) -> Expr {
    markup.as_expr()
}

/// Convert a canonical or compatibility article expression into markup.
///
/// # Errors
///
/// Returns an error when the expression is neither a `MarkupDoc` value nor a
/// compatibility article value.
pub fn markup_from_article(doc: &Expr) -> Result<MarkupDoc> {
    match MarkupDoc::from_expr(doc) {
        Ok(markup) => Ok(markup),
        Err(_) => compatibility_to_markup(doc),
    }
}

/// Build an article document from a title and an ordered list of blocks.
pub fn article(title: &str, blocks: Vec<Expr>) -> Expr {
    let compatibility = compatibility_article(title, blocks);
    match compatibility_to_markup(&compatibility) {
        Ok(markup) => article_from_markup(&markup),
        Err(_) => compatibility,
    }
}

fn compatibility_article(title: &str, blocks: Vec<Expr>) -> Expr {
    map(vec![
        ("class", Expr::Symbol(Symbol::qualified("doc", "Article"))),
        ("title", Expr::String(title.to_owned())),
        ("blocks", Expr::List(blocks)),
    ])
}

/// The article title.
pub fn title(doc: &Expr) -> Option<String> {
    if let Ok(markup) = MarkupDoc::from_expr(doc) {
        return markup.title;
    }
    compatibility_title(doc)
}

fn compatibility_title(doc: &Expr) -> Option<String> {
    match field(doc, "title") {
        Some(Expr::String(text)) => Some(text.clone()),
        _ => None,
    }
}

/// The article's ordered blocks.
pub fn blocks(doc: &Expr) -> Vec<Expr> {
    if let Ok(markup) = MarkupDoc::from_expr(doc) {
        return markup
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| markup_block_to_compatibility(index, block))
            .collect();
    }
    compatibility_blocks(doc)
}

fn compatibility_blocks(doc: &Expr) -> Vec<Expr> {
    match field(doc, "blocks") {
        Some(Expr::List(items)) => items.clone(),
        _ => Vec::new(),
    }
}

/// The kind of a block (the `block` tag), if present.
pub fn block_kind(block: &Expr) -> Option<Symbol> {
    match field(block, BLOCK_KEY) {
        Some(Expr::Symbol(symbol)) => Some(symbol.clone()),
        _ => None,
    }
}

fn block(kind: &str, mut entries: Vec<(&str, Expr)>) -> Expr {
    let mut pairs = vec![(BLOCK_KEY, Expr::Symbol(Symbol::new(kind)))];
    pairs.append(&mut entries);
    map(pairs)
}

/// A section heading block.
pub fn section(heading: &str) -> Expr {
    block("section", vec![("title", Expr::String(heading.to_owned()))])
}

/// A prose paragraph block.
pub fn prose(text: &str) -> Expr {
    block("prose", vec![("text", Expr::String(text.to_owned()))])
}

/// An equation block carrying its source (for example TeX).
pub fn equation(source: &str) -> Expr {
    block("equation", vec![("tex", Expr::String(source.to_owned()))])
}

/// A figure block.
pub fn figure(caption: &str, src: &str) -> Expr {
    block(
        "figure",
        vec![
            ("caption", Expr::String(caption.to_owned())),
            ("src", Expr::String(src.to_owned())),
        ],
    )
}

/// A table block from rows of cell values.
pub fn table(rows: Vec<Vec<Expr>>) -> Expr {
    let rows = rows.into_iter().map(Expr::List).collect();
    block("table", vec![("rows", Expr::List(rows))])
}

/// A citation block.
pub fn citation(cite_key: &str, text: &str) -> Expr {
    block(
        "citation",
        vec![
            ("key", Expr::Symbol(Symbol::new(cite_key))),
            ("text", Expr::String(text.to_owned())),
        ],
    )
}

/// An embedded-runtime block: a runtime `value` rendered by `lens` inside the
/// article. Cached output (a precomputed Scene) may be attached later.
pub fn embed_block(value: Expr, lens: &str) -> Expr {
    block(
        "embed",
        vec![("value", value), ("lens", Expr::Symbol(Symbol::new(lens)))],
    )
}

fn compatibility_to_markup(doc: &Expr) -> Result<MarkupDoc> {
    let title = compatibility_title(doc)
        .ok_or_else(|| Error::Eval("article is missing title".to_owned()))?;
    let blocks = compatibility_blocks(doc);
    let blocks = blocks
        .iter()
        .map(compatibility_block_to_markup)
        .collect::<Result<Vec<_>>>()?;
    Ok(MarkupDoc {
        title: Some(title),
        blocks,
        attrs: Default::default(),
        source: None,
    })
}

fn compatibility_block_to_markup(block: &Expr) -> Result<MarkupBlock> {
    match block_kind(block)
        .as_ref()
        .map(|kind| kind.name.to_string())
        .as_deref()
    {
        Some("section") => Ok(MarkupBlock::Heading {
            level: 2,
            text: vec![Inline::Text(string_text(block, "title"))],
            id: None,
            span: None,
        }),
        Some("prose") => Ok(MarkupBlock::Paragraph {
            content: vec![Inline::Text(string_text(block, "text"))],
            span: None,
        }),
        Some("equation") => Ok(MarkupBlock::MathBlock {
            source: MathSource {
                notation: "tex".to_owned(),
                text: string_text(block, "tex"),
            },
            span: None,
        }),
        Some("figure") => Ok(MarkupBlock::Figure {
            src: string_text(block, "src"),
            caption: vec![Inline::Text(string_text(block, "caption"))],
            span: None,
        }),
        Some("table") => Ok(table_to_markup(block)),
        Some("citation") => Ok(MarkupBlock::Raw {
            backend: BackendId::new("view-doc/citation"),
            text: string_text(block, "text"),
            span: None,
        }),
        Some("embed") => Ok(MarkupBlock::Raw {
            backend: BackendId::new("view-doc/embed"),
            text: field(block, "value").map(expr_text).unwrap_or_default(),
            span: None,
        }),
        Some(other) => Ok(MarkupBlock::Raw {
            backend: BackendId::new(format!("view-doc/{other}")),
            text: expr_text(block),
            span: None,
        }),
        None => Err(Error::Eval(
            "article block is missing block kind".to_owned(),
        )),
    }
}

fn table_to_markup(block: &Expr) -> MarkupBlock {
    let rows = match field(block, "rows") {
        Some(Expr::List(rows)) => rows
            .iter()
            .map(|row| match row {
                Expr::List(cells) => cells
                    .iter()
                    .map(|cell| vec![Inline::Text(expr_text(cell))])
                    .collect(),
                other => vec![vec![Inline::Text(expr_text(other))]],
            })
            .collect::<Vec<Vec<Vec<Inline>>>>(),
        _ => Vec::new(),
    };
    let mut rows = rows.into_iter();
    MarkupBlock::Table {
        header: rows.next().unwrap_or_default(),
        rows: rows.collect(),
        span: None,
    }
}

fn markup_block_to_compatibility(index: usize, markup_block: &MarkupBlock) -> Expr {
    match markup_block {
        MarkupBlock::Heading { text, .. } => section(&inline_text(text)),
        MarkupBlock::Paragraph { content, .. } => prose(&inline_text(content)),
        MarkupBlock::MathBlock { source, .. } => equation(&source.text),
        MarkupBlock::Figure { src, caption, .. } => figure(&inline_text(caption), src),
        MarkupBlock::Table { header, rows, .. } => {
            let all_rows = std::iter::once(header)
                .chain(rows.iter())
                .map(|row| {
                    row.iter()
                        .map(|cell| Expr::String(inline_text(cell)))
                        .collect()
                })
                .collect();
            table(all_rows)
        }
        MarkupBlock::Raw { backend, text, .. } if backend.as_str() == "view-doc/citation" => {
            citation("citation", text)
        }
        MarkupBlock::Raw { backend, text, .. } if backend.as_str() == "view-doc/embed" => {
            embed_block(Expr::String(text.clone()), "view:default")
        }
        MarkupBlock::CodeBlock { code, .. } => prose(code),
        MarkupBlock::Quote { blocks, .. } => prose(
            &blocks
                .iter()
                .map(block_summary)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        MarkupBlock::List { items, .. } => {
            let text = items
                .iter()
                .map(|item| item.iter().map(block_summary).collect::<Vec<_>>().join(" "))
                .collect::<Vec<_>>()
                .join("\n");
            prose(&text)
        }
        MarkupBlock::Raw { text, .. } => block(
            "raw",
            vec![
                ("key", Expr::String(format!("raw-{index}"))),
                ("text", Expr::String(text.clone())),
            ],
        ),
    }
}

fn block_summary(block: &MarkupBlock) -> String {
    match block {
        MarkupBlock::Heading { text, .. } => inline_text(text),
        MarkupBlock::Paragraph { content, .. } => inline_text(content),
        MarkupBlock::CodeBlock { code, .. } => code.clone(),
        MarkupBlock::MathBlock { source, .. } => source.text.clone(),
        MarkupBlock::Quote { blocks, .. } => blocks
            .iter()
            .map(block_summary)
            .collect::<Vec<_>>()
            .join("\n"),
        MarkupBlock::List { items, .. } => items
            .iter()
            .map(|item| item.iter().map(block_summary).collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n"),
        MarkupBlock::Table { header, rows, .. } => std::iter::once(header)
            .chain(rows.iter())
            .flat_map(|row| row.iter().map(|cell| inline_text(cell)))
            .collect::<Vec<_>>()
            .join(" "),
        MarkupBlock::Figure { src, caption, .. } => format!("{} {}", inline_text(caption), src)
            .trim()
            .to_owned(),
        MarkupBlock::Raw { text, .. } => text.clone(),
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

fn string_text(expr: &Expr, name: &str) -> String {
    match field(expr, name) {
        Some(Expr::String(text)) => text.clone(),
        Some(other) => expr_text(other),
        None => String::new(),
    }
}

fn expr_text(expr: &Expr) -> String {
    match expr {
        Expr::String(text) => text.clone(),
        Expr::Symbol(symbol) => symbol.to_string(),
        Expr::Number(number) => number.canonical.clone(),
        Expr::Bool(value) => value.to_string(),
        Expr::Nil => String::new(),
        other => format!("{other:?}"),
    }
}
