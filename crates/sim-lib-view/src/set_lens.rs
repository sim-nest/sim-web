//! `intent/set-lens` handling and the per-pane active-lens state.
//!
//! Switching the active lens for a pane is data, not a rebuild: the active lens
//! per pane is a small SIM value (a map of pane id to lens id). Applying an
//! `intent/set-lens` returns an updated state value and never touches the value
//! being shown, so the choice persists in the workspace value (P6) by virtue of
//! being a value itself.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_intent::{field, intent_kind_of};

/// An empty pane-lens state.
pub fn empty_pane_lenses() -> Expr {
    Expr::Map(Vec::new())
}

/// The active lens for `pane`, if one has been chosen.
pub fn active_lens(state: &Expr, pane: &str) -> Option<Symbol> {
    let Expr::Map(entries) = state else {
        return None;
    };
    entries.iter().find_map(|(key, value)| {
        let matches = matches!(key, Expr::Symbol(symbol) if &*symbol.name == pane);
        match value {
            Expr::Symbol(lens) if matches => Some(lens.clone()),
            _ => None,
        }
    })
}

/// Apply an `intent/set-lens`, returning the updated pane-lens state. The shown
/// value is never read or written here; only the pane's active lens changes.
pub fn apply_set_lens(state: &Expr, intent: &Expr) -> Result<Expr> {
    match intent_kind_of(intent) {
        Some(kind) if &*kind.name == "set-lens" => {}
        _ => {
            return Err(Error::HostError(
                "apply_set_lens expects an intent/set-lens".to_owned(),
            ));
        }
    }
    let pane = match field(intent, "pane") {
        Some(Expr::Symbol(symbol)) => symbol.clone(),
        _ => {
            return Err(Error::HostError(
                "intent/set-lens 'pane' must be a symbol".to_owned(),
            ));
        }
    };
    let lens = match field(intent, "lens") {
        Some(Expr::Symbol(symbol)) => symbol.clone(),
        _ => {
            return Err(Error::HostError(
                "intent/set-lens 'lens' must be a symbol".to_owned(),
            ));
        }
    };
    let mut entries = match state {
        Expr::Map(entries) => entries.clone(),
        _ => Vec::new(),
    };
    let key_matches = |key: &Expr| matches!(key, Expr::Symbol(symbol) if symbol == &pane);
    if let Some(slot) = entries.iter_mut().find(|(key, _)| key_matches(key)) {
        slot.1 = Expr::Symbol(lens);
    } else {
        entries.push((Expr::Symbol(pane), Expr::Symbol(lens)));
    }
    Ok(Expr::Map(entries))
}
