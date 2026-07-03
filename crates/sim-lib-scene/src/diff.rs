//! Scene diff and patch application.
//!
//! A scene diff is itself a scene value (a `scene/patch` node), so it can be
//! snapshotted, sent over the wire, or replayed like any other Scene. [`diff`]
//! produces a patch that records the minimal set-and-remove operations turning
//! `old` into `new`; [`apply`] replays a patch onto a scene. The contract is
//! exactness: `apply(old, diff(old, new))` reconstructs `new` exactly.
//!
//! The diff descends through equal-length sequences and through map keys, so
//! moving or editing one node re-emits only that node. Length-changing
//! sequences and type changes fall back to a whole-value set at that path,
//! which still reconstructs exactly. Maps whose keys are REORDERED (the same
//! keys in a new order) also fall back to a whole-value set: a key-matched
//! descent emits no ops for a pure reorder, yet `apply` preserves the old key
//! order, so a fallback is required for `apply(old, diff(old,new)) == new` to
//! hold exactly. Path addressing (the `k`/`i` wire form and the
//! navigate/set/remove logic) is the shared `sim_value::path` primitive.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_value::path::{Path, PathError, Segment, remove_at, set_at};

use crate::model::node;

const OP_KEY: &str = "op";
const PATH_KEY: &str = "path";
const VALUE_KEY: &str = "value";
const OPS_KEY: &str = "ops";
const OP_SET: &str = "set";
const OP_REMOVE: &str = "remove";

enum Op {
    Set { path: Path, value: Expr },
    Remove { path: Path },
}

/// Build the patch that turns `old` into `new`.
pub fn diff(old: &Expr, new: &Expr) -> Expr {
    let mut ops = Vec::new();
    diff_value(old, new, &mut Path::new(), &mut ops);
    let op_exprs = ops.into_iter().map(op_to_expr).collect();
    node("patch", vec![(OPS_KEY, Expr::List(op_exprs))])
}

/// Apply `patch` to `scene`, returning the reconstructed scene.
pub fn apply(scene: &Expr, patch: &Expr) -> Result<Expr> {
    let ops = parse_ops(patch)?;
    let mut result = scene.clone();
    for op in ops {
        result = match op {
            Op::Set { path, value } => set_at(&result, &path, value).map_err(map_path_error)?,
            Op::Remove { path } => remove_at(&result, &path).map_err(map_path_error)?,
        };
    }
    Ok(result)
}

fn diff_value(old: &Expr, new: &Expr, path: &mut Path, ops: &mut Vec<Op>) {
    // Compare STRUCTURALLY, not with `==`: `Expr`'s equality is canonical and
    // ignores map key order, which would hide a pure reorder from the differ and
    // leave `apply` reconstructing the old order.
    if structural_eq(old, new) {
        return;
    }
    match (old, new) {
        (Expr::Map(old_entries), Expr::Map(new_entries)) => {
            // A pure key reorder (same keys, new order) yields zero per-key ops
            // but `apply` would keep the old order. Re-emit the whole map so the
            // new order is reconstructed exactly.
            if same_keys_reordered(old_entries, new_entries) {
                ops.push(Op::Set {
                    path: path.clone(),
                    value: new.clone(),
                });
                return;
            }
            for (key, old_value) in old_entries {
                match find_value(new_entries, key) {
                    Some(new_value) => {
                        path.0.push(Segment::Key(key.clone()));
                        diff_value(old_value, new_value, path, ops);
                        path.0.pop();
                    }
                    None => {
                        path.0.push(Segment::Key(key.clone()));
                        ops.push(Op::Remove { path: path.clone() });
                        path.0.pop();
                    }
                }
            }
            for (key, new_value) in new_entries {
                if find_value(old_entries, key).is_none() {
                    path.0.push(Segment::Key(key.clone()));
                    ops.push(Op::Set {
                        path: path.clone(),
                        value: new_value.clone(),
                    });
                    path.0.pop();
                }
            }
        }
        (Expr::List(old_items), Expr::List(new_items))
        | (Expr::Vector(old_items), Expr::Vector(new_items))
        | (Expr::Set(old_items), Expr::Set(new_items))
            if old_items.len() == new_items.len() =>
        {
            for (index, (old_item, new_item)) in old_items.iter().zip(new_items).enumerate() {
                path.0.push(Segment::Index(index));
                diff_value(old_item, new_item, path, ops);
                path.0.pop();
            }
        }
        _ => ops.push(Op::Set {
            path: path.clone(),
            value: new.clone(),
        }),
    }
}

fn find_value<'a>(entries: &'a [(Expr, Expr)], key: &Expr) -> Option<&'a Expr> {
    entries
        .iter()
        .find_map(|(entry_key, value)| (entry_key == key).then_some(value))
}

/// Order-sensitive equality. Unlike `Expr::eq` (canonical, which ignores map
/// key order and set/sequence ordering), this treats a reordering as a
/// difference, so the differ can reconstruct the EXACT structure of `new`.
fn structural_eq(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Map(ae), Expr::Map(be)) => {
            ae.len() == be.len()
                && ae
                    .iter()
                    .zip(be)
                    .all(|((ak, av), (bk, bv))| structural_eq(ak, bk) && structural_eq(av, bv))
        }
        (Expr::List(ai), Expr::List(bi))
        | (Expr::Vector(ai), Expr::Vector(bi))
        | (Expr::Set(ai), Expr::Set(bi)) => {
            ai.len() == bi.len() && ai.iter().zip(bi).all(|(x, y)| structural_eq(x, y))
        }
        _ => a == b,
    }
}

/// True when `old` and `new` carry exactly the same keys (as a set) but in a
/// different order. Add/remove cases (different key sets) are left to the
/// per-key set/remove loops, which reconstruct exactly for them.
fn same_keys_reordered(old: &[(Expr, Expr)], new: &[(Expr, Expr)]) -> bool {
    if old.len() != new.len() {
        return false;
    }
    let order_differs = old
        .iter()
        .zip(new)
        .any(|((old_key, _), (new_key, _))| old_key != new_key);
    if !order_differs {
        return false;
    }
    // Same length and order differs: confirm the key SETS match (otherwise it is
    // an add+remove of equal count, handled by the per-key loops).
    old.iter()
        .all(|(old_key, _)| find_value(new, old_key).is_some())
        && new
            .iter()
            .all(|(new_key, _)| find_value(old, new_key).is_some())
}

fn patch_error(message: &str) -> Error {
    Error::HostError(format!("scene patch apply error: {message}"))
}

fn map_path_error(error: PathError) -> Error {
    Error::HostError(format!("scene patch apply error: {error:?}"))
}

fn op_to_expr(op: Op) -> Expr {
    let mut entries = Vec::new();
    match op {
        Op::Set { path, value } => {
            entries.push(sym_entry(OP_KEY, Expr::Symbol(Symbol::new(OP_SET))));
            entries.push(sym_entry(PATH_KEY, path.to_expr()));
            entries.push(sym_entry(VALUE_KEY, value));
        }
        Op::Remove { path } => {
            entries.push(sym_entry(OP_KEY, Expr::Symbol(Symbol::new(OP_REMOVE))));
            entries.push(sym_entry(PATH_KEY, path.to_expr()));
        }
    }
    Expr::Map(entries)
}

fn sym_entry(key: &str, value: Expr) -> (Expr, Expr) {
    (Expr::Symbol(Symbol::new(key)), value)
}

fn parse_ops(patch: &Expr) -> Result<Vec<Op>> {
    let Expr::Map(entries) = patch else {
        return Err(patch_error("patch is not a map"));
    };
    let ops_expr = find_value(entries, &Expr::Symbol(Symbol::new(OPS_KEY)))
        .ok_or_else(|| patch_error("patch is missing an 'ops' entry"))?;
    let Expr::List(op_exprs) = ops_expr else {
        return Err(patch_error("patch 'ops' is not a list"));
    };
    op_exprs.iter().map(parse_op).collect()
}

fn parse_op(op: &Expr) -> Result<Op> {
    let Expr::Map(entries) = op else {
        return Err(patch_error("op is not a map"));
    };
    let op_name = match find_value(entries, &Expr::Symbol(Symbol::new(OP_KEY))) {
        Some(Expr::Symbol(symbol)) => symbol.name.clone(),
        _ => return Err(patch_error("op is missing an 'op' symbol")),
    };
    let path = match find_value(entries, &Expr::Symbol(Symbol::new(PATH_KEY))) {
        Some(path_expr) => Path::from_expr(path_expr).map_err(map_path_error)?,
        None => return Err(patch_error("op is missing a 'path'")),
    };
    match &*op_name {
        OP_SET => {
            let value = find_value(entries, &Expr::Symbol(Symbol::new(VALUE_KEY)))
                .ok_or_else(|| patch_error("set op is missing a 'value'"))?
                .clone();
            Ok(Op::Set { path, value })
        }
        OP_REMOVE => Ok(Op::Remove { path }),
        other => Err(patch_error(&format!("unknown op '{other}'"))),
    }
}
