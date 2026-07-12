//! The document value model.
//!
//! A scientific article is a round-trippable SIM value: a map of a title and an
//! ordered list of semantic blocks. Blocks are tagged with a `block` key (not
//! `kind`, which is reserved for scene nodes) so a document is plain data,
//! distinct from the Scene a lens renders from it. Block kinds: section, prose,
//! equation, figure, table, citation, and embed (an embedded runtime value).

use sim_kernel::{Expr, Symbol};

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

/// Build an article document from a title and an ordered list of blocks.
pub fn article(title: &str, blocks: Vec<Expr>) -> Expr {
    map(vec![
        ("class", Expr::Symbol(Symbol::qualified("doc", "Article"))),
        ("title", Expr::String(title.to_owned())),
        ("blocks", Expr::List(blocks)),
    ])
}

/// The article title.
pub fn title(doc: &Expr) -> Option<String> {
    match field(doc, "title") {
        Some(Expr::String(text)) => Some(text.clone()),
        _ => None,
    }
}

/// The article's ordered blocks.
pub fn blocks(doc: &Expr) -> Vec<Expr> {
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
