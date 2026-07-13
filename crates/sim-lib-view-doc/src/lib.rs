//! Markup article workspace lens for the SIM Web UI.
//!
//! The article lens is a real authoring surface, not a markdown textarea: a
//! round-trippable document value with semantic blocks (section, prose,
//! equation, figure, table, citation, embedded-runtime block), an outline plus
//! block canvas, side-by-side source and formatted views as two lenses on the
//! same document value, and embedded live blocks via `scene/embed`.
//!
//! [`doc`] adapts article-shaped compatibility helpers to `MarkupDoc`; [`lens`]
//! is the formatted and source lenses plus export over that shared markup value.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod citizen;
pub mod doc;
pub mod lens;

pub use citizen::{DocArticleDescriptor, doc_article_class_symbol};
pub use doc::{
    ARTICLE_CLASS, article, article_from_markup, blocks, citation, embed_block, equation, figure,
    markup_from_article, prose, section, table, title,
};
pub use lens::{
    ARTICLE_FORMATTED_LENS, ARTICLE_SOURCE_LENS, article_formatted, article_outline,
    article_source, export_intermediate, export_markdown, markup_edit_from_intent, with_cache,
};

/// Stable symbol for the scientific article lens.
pub const ARTICLE_LENS: &str = ARTICLE_FORMATTED_LENS;

/// Embedded cookbook recipe books shipped with this library.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
