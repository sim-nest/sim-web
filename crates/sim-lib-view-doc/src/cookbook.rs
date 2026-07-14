//! Deterministic cookbook builders for document view recipes.

use sim_kernel::Expr;
use sim_lib_scene::stack;

use crate::{article, article_formatted, article_source, equation, prose, section};

/// Build the formatted article Scene used by the cookbook recipe.
pub fn article_demo() -> Expr {
    let scene = article_formatted(&sample_article());
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

/// Build paired formatted/source Scenes for the LaTeX lens cookbook recipe.
pub fn latex_lens_demo() -> Expr {
    let doc = sample_article();
    let scene = stack("row", vec![article_formatted(&doc), article_source(&doc)]);
    debug_assert!(sim_lib_scene::validate_scene(&scene).is_ok());
    scene
}

fn sample_article() -> Expr {
    article(
        "Cookbook Article",
        vec![
            section("Surface"),
            prose("The same document opens through formatted and source lenses."),
            equation("\\int_0^1 x^2 dx = 1/3"),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookbook_doc_demos_are_valid_scenes() {
        sim_lib_scene::validate_scene(&article_demo()).expect("article scene validates");
        sim_lib_scene::validate_scene(&latex_lens_demo()).expect("latex scene validates");
    }
}
