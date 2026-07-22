//! Deterministic cookbook builders for Intent recipes.

use sim_kernel::{Expr, Symbol};

use crate::{Origin, intent, validate_intent};

/// Build the select Intent used by the cookbook selection recipe.
pub fn select_intent_demo() -> Expr {
    let intent = intent(
        "select",
        Origin::human(1),
        vec![(
            "targets",
            Expr::List(vec![Expr::Symbol(Symbol::qualified("pane", "main"))]),
        )],
    );
    debug_assert!(validate_intent(&intent).is_ok());
    intent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{intent_kind_of, referenced_targets};

    #[test]
    fn select_intent_demo_is_valid_and_names_a_target() {
        let demo = select_intent_demo();
        validate_intent(&demo).expect("demo Intent validates");
        assert_eq!(
            intent_kind_of(&demo).map(|kind| kind.name.to_string()),
            Some("select".to_owned())
        );
        assert_eq!(
            referenced_targets(&demo),
            vec![(
                "targets[0]".to_owned(),
                Expr::Symbol(Symbol::qualified("pane", "main")),
            )]
        );
    }
}
