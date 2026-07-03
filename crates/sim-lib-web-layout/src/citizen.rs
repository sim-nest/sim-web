use sim_citizen_derive::Citizen;
use sim_kernel::{Expr, Result, Symbol};

use crate::new_workspace;

/// A validated workspace layout wrapped as a runtime Citizen object.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "web/Workspace", version = 1)]
pub struct WorkspaceDescriptor {
    #[citizen(with = "workspace_expr")]
    workspace: Expr,
}

impl WorkspaceDescriptor {
    /// Builds a descriptor from a workspace expression, validating it.
    ///
    /// # Errors
    ///
    /// Returns an error when `workspace` is not a well-formed workspace
    /// expression.
    pub fn from_expr(workspace: Expr) -> Result<Self> {
        workspace_expr::decode(&workspace)?;
        Ok(Self { workspace })
    }

    /// Returns the underlying workspace expression.
    pub fn as_expr(&self) -> &Expr {
        &self.workspace
    }
}

impl Default for WorkspaceDescriptor {
    fn default() -> Self {
        Self::from_expr(new_workspace("builder"))
            .expect("default workspace descriptor should be valid")
    }
}

/// Returns the class symbol for the workspace descriptor Citizen.
pub fn workspace_descriptor_class_symbol() -> Symbol {
    Symbol::qualified("web", "Workspace")
}

pub(crate) mod workspace_expr {
    use sim_kernel::{Error, Expr, Result};

    use crate::WORKSPACE_CLASS;

    pub fn encode(expr: &Expr) -> Expr {
        expr.clone()
    }

    pub fn decode(expr: &Expr) -> Result<Expr> {
        let Expr::Map(entries) = expr else {
            return Err(Error::Eval("workspace descriptor must be a map".to_owned()));
        };
        let class = entries.iter().find_map(|(key, value)| {
            matches!(key, Expr::Symbol(symbol) if symbol.namespace.is_none() && symbol.name.as_ref() == "class")
                .then_some(value)
        });
        match class {
            Some(Expr::Symbol(symbol)) if symbol.to_string() == WORKSPACE_CLASS => Ok(expr.clone()),
            _ => Err(Error::Eval(
                "workspace descriptor class must be web/Workspace".to_owned(),
            )),
        }
    }
}
