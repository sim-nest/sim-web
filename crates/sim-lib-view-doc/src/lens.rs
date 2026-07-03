//! The article lenses: formatted view, source view, and export.
//!
//! Two lenses render the same document value side by side -- a formatted view
//! (outline plus a block canvas) and a source view (each block's raw value).
//! Embedded live blocks render through `scene/embed`: a runtime value drawn by
//! its own lens inside the article, using a cached output when one is attached.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{node, sym};
use sim_lib_view::{Mode, render_value, universal_scene};

use crate::doc::{block_kind, blocks, field, title};

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
    let canvas = blocks(doc).iter().map(block_formatted).collect();
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

/// Render the source article: the title and each block's raw value as text.
pub fn article_source(doc: &Expr) -> Expr {
    let mut children = vec![text(format!(
        "# {}",
        title(doc).unwrap_or_else(|| "untitled".to_owned())
    ))];
    for block in blocks(doc) {
        children.push(text(render_value(&block)));
    }
    node(
        "stack",
        vec![
            ("role", sym("source")),
            ("dir", sym("column")),
            ("children", Expr::List(children)),
        ],
    )
}

/// The outline: a tree of the article's section headings.
pub fn article_outline(doc: &Expr) -> Expr {
    let sections = blocks(doc)
        .iter()
        .filter(|block| block_kind(block).as_ref().map(|k| k.name.as_ref()) == Some("section"))
        .map(|block| {
            let heading = match field(block, "title") {
                Some(Expr::String(text)) => text.clone(),
                _ => "section".to_owned(),
            };
            text(heading)
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

fn block_formatted(block: &Expr) -> Expr {
    match block_kind(block)
        .as_ref()
        .map(|k| k.name.to_string())
        .as_deref()
    {
        Some("section") => node(
            "text",
            vec![
                ("role", sym("heading")),
                ("text", string_field(block, "title")),
            ],
        ),
        Some("prose") => node("text", vec![("text", string_field(block, "text"))]),
        Some("equation") => node(
            "box",
            vec![
                ("role", sym("equation")),
                (
                    "children",
                    Expr::List(vec![node(
                        "text",
                        vec![("text", string_field(block, "tex"))],
                    )]),
                ),
            ],
        ),
        Some("figure") => node(
            "box",
            vec![
                ("role", sym("figure")),
                (
                    "children",
                    Expr::List(vec![node(
                        "text",
                        vec![("text", string_field(block, "caption"))],
                    )]),
                ),
            ],
        ),
        Some("table") => table_scene(block),
        Some("citation") => node(
            "text",
            vec![
                ("role", sym("citation")),
                ("text", string_field(block, "text")),
            ],
        ),
        Some("embed") => embed_scene(block),
        _ => node(
            "text",
            vec![("text", Expr::String("[unknown block]".to_owned()))],
        ),
    }
}

fn embed_scene(block: &Expr) -> Expr {
    let lens = match field(block, "lens") {
        Some(Expr::Symbol(symbol)) => symbol.clone(),
        _ => Symbol::new("view:default"),
    };
    // Use a cached output if one is attached, else render the value live.
    let inner = match field(block, "cache") {
        Some(cached) => cached.clone(),
        None => universal_scene(field(block, "value").unwrap_or(&Expr::Nil), Mode::Builder),
    };
    node(
        "embed",
        vec![("lens", Expr::Symbol(lens)), ("scene", inner)],
    )
}

fn table_scene(block: &Expr) -> Expr {
    let rows = match field(block, "rows") {
        Some(Expr::List(rows)) => rows
            .iter()
            .map(|row| match row {
                Expr::List(cells) => Expr::List(
                    cells
                        .iter()
                        .map(|cell| Expr::String(render_value(cell)))
                        .collect(),
                ),
                other => Expr::List(vec![Expr::String(render_value(other))]),
            })
            .collect(),
        _ => Vec::new(),
    };
    node("table", vec![("rows", Expr::List(rows))])
}

/// The stable intermediate document value used for export. Export formats are
/// derived from this single IR; here it is the normalized document value.
pub fn export_intermediate(doc: &Expr) -> Expr {
    crate::doc::article(
        &title(doc).unwrap_or_else(|| "untitled".to_owned()),
        blocks(doc),
    )
}

/// A Markdown export stub over the stable intermediate document value.
pub fn export_markdown(doc: &Expr) -> String {
    let mut out = format!(
        "# {}\n\n",
        title(doc).unwrap_or_else(|| "untitled".to_owned())
    );
    for block in blocks(doc) {
        let line = match block_kind(&block)
            .as_ref()
            .map(|k| k.name.to_string())
            .as_deref()
        {
            Some("section") => format!("## {}\n\n", string_text(&block, "title")),
            Some("prose") => format!("{}\n\n", string_text(&block, "text")),
            Some("equation") => format!("$$\n{}\n$$\n\n", string_text(&block, "tex")),
            Some("citation") => format!("> {}\n\n", string_text(&block, "text")),
            Some("embed") => "[embedded runtime value]\n\n".to_owned(),
            _ => String::new(),
        };
        out.push_str(&line);
    }
    out
}

fn string_field(block: &Expr, name: &str) -> Expr {
    Expr::String(string_text(block, name))
}

fn string_text(block: &Expr, name: &str) -> String {
    match field(block, name) {
        Some(Expr::String(text)) => text.clone(),
        Some(other) => render_value(other),
        None => String::new(),
    }
}

fn text(content: String) -> Expr {
    node("text", vec![("text", Expr::String(content))])
}
