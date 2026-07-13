use sim_citizen_derive::Citizen;
use sim_kernel::{Expr, Result, Symbol};

use crate::{article, prose};

/// A validated documentation article wrapped as a runtime Citizen object.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "doc/Article", version = 1)]
pub struct DocArticleDescriptor {
    #[citizen(with = "article_expr")]
    article: Expr,
}

impl DocArticleDescriptor {
    /// Builds a descriptor from an article expression, validating it.
    ///
    /// # Errors
    ///
    /// Returns an error when `article` is not a well-formed article expression.
    pub fn from_expr(article: Expr) -> Result<Self> {
        article_expr::decode(&article)?;
        Ok(Self { article })
    }

    /// Returns the underlying article expression.
    pub fn as_expr(&self) -> &Expr {
        &self.article
    }
}

impl Default for DocArticleDescriptor {
    fn default() -> Self {
        Self::from_expr(article("Citizen Article", vec![prose("citizen doc")]))
            .expect("default article descriptor should be valid")
    }
}

/// Returns the class symbol for the documentation article Citizen.
pub fn doc_article_class_symbol() -> Symbol {
    Symbol::qualified("doc", "Article")
}

pub(crate) mod article_expr {
    use sim_kernel::{Error, Expr, Result};

    use crate::markup_from_article;

    pub fn encode(expr: &Expr) -> Expr {
        expr.clone()
    }

    pub fn decode(expr: &Expr) -> Result<Expr> {
        markup_from_article(expr)
            .map_err(|error| Error::Eval(format!("malformed doc article: {error}")))?;
        Ok(expr.clone())
    }
}
