use sim_citizen_derive::Citizen;
use sim_kernel::{Expr, Result, Symbol};

use crate::node;

/// A validated Scene wrapped as a runtime Citizen object.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "scene/Scene", version = 1)]
pub struct SceneDescriptor {
    #[citizen(with = "scene_expr")]
    scene: Expr,
}

impl SceneDescriptor {
    /// Builds a descriptor from a Scene expression, validating it.
    ///
    /// # Errors
    ///
    /// Returns an error when `scene` is not a well-formed Scene expression.
    pub fn from_expr(scene: Expr) -> Result<Self> {
        scene_expr::decode(&scene)?;
        Ok(Self { scene })
    }

    /// Returns the underlying Scene expression.
    pub fn as_expr(&self) -> &Expr {
        &self.scene
    }
}

impl Default for SceneDescriptor {
    fn default() -> Self {
        Self::from_expr(node(
            "text",
            vec![("text", Expr::String("citizen scene".to_owned()))],
        ))
        .expect("default scene descriptor should be valid")
    }
}

/// Returns the class symbol for the Scene descriptor Citizen.
pub fn scene_descriptor_class_symbol() -> Symbol {
    Symbol::qualified("scene", "Scene")
}

pub(crate) mod scene_expr {
    use sim_kernel::{Error, Expr, Result};

    use crate::validate_scene;

    pub fn encode(expr: &Expr) -> Expr {
        expr.clone()
    }

    pub fn decode(expr: &Expr) -> Result<Expr> {
        validate_scene(expr).map_err(|error| Error::Eval(format!("malformed scene: {error}")))?;
        Ok(expr.clone())
    }
}
