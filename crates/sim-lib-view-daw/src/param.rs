//! Parameter and transport edits: `intent/set-param` and `intent/scrub`.
//!
//! These produce the new parameter or transport value to commit through
//! `realize`; nothing is mutated in place. They back onto the existing DAW and
//! plugin/synth values: a synth patch is a parameter map, a transport carries a
//! playhead position.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_intent::{field, intent_kind_of};

/// Apply an `intent/set-param` to a parameter map, returning the new map. The
/// returned value is the operation to commit through `realize`.
pub fn apply_set_param(params: &Expr, intent: &Expr) -> Result<Expr> {
    expect_kind(intent, "set-param")?;
    let param = match field(intent, "param") {
        Some(Expr::Symbol(symbol)) => symbol.clone(),
        _ => {
            return Err(Error::HostError(
                "set-param 'param' must be a symbol".to_owned(),
            ));
        }
    };
    let value = field(intent, "value")
        .cloned()
        .ok_or_else(|| Error::HostError("set-param is missing a 'value'".to_owned()))?;
    Ok(set_key(params, &param, value))
}

/// Apply an `intent/scrub` to a transport value, returning the new transport
/// with its playhead moved to the requested position.
pub fn apply_scrub(transport: &Expr, intent: &Expr) -> Result<Expr> {
    expect_kind(intent, "scrub")?;
    let at = field(intent, "at")
        .cloned()
        .ok_or_else(|| Error::HostError("scrub is missing an 'at'".to_owned()))?;
    Ok(set_key(transport, &Symbol::new("position"), at))
}

fn expect_kind(intent: &Expr, kind: &str) -> Result<()> {
    match intent_kind_of(intent) {
        Some(symbol) if symbol.name.as_ref() == kind => Ok(()),
        _ => Err(Error::HostError(format!("expected an intent/{kind}"))),
    }
}

fn set_key(map: &Expr, key: &Symbol, value: Expr) -> Expr {
    let mut entries = match map {
        Expr::Map(entries) => entries.clone(),
        _ => Vec::new(),
    };
    let matches = |entry_key: &Expr| matches!(entry_key, Expr::Symbol(symbol) if symbol == key);
    if let Some(slot) = entries.iter_mut().find(|(entry_key, _)| matches(entry_key)) {
        slot.1 = value;
    } else {
        entries.push((Expr::Symbol(key.clone()), value));
    }
    Expr::Map(entries)
}
