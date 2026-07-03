//! History, snapshots, the session log, and review primitives -- all as values.
//!
//! Undo/redo is not a bespoke command stack: each edit is recorded as a forward
//! operation and its inverse operation (the same `set-value` op pointing at the
//! prior value), reusing event/effect ledger semantics. Undoing replays the
//! inverse through `realize`; redoing replays the forward. Snapshots, the edit
//! log, and annotations are plain SIM values, so prior states can be inspected
//! and restored as data.

use std::collections::BTreeMap;

use sim_kernel::{Error, Expr, Result, Symbol};

use crate::transport::Transport;

fn set_value_op(value: Expr) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("op")),
            Expr::Symbol(Symbol::new("set-value")),
        ),
        (Expr::Symbol(Symbol::new("value")), value),
    ])
}

/// One ledger entry: the resource and the forward/inverse operations.
#[derive(Clone, Debug)]
struct LedgerEntry {
    resource: Symbol,
    forward: Expr,
    inverse: Expr,
}

/// An undo/redo history recorded as inverse operations in a value-backed ledger.
#[derive(Default)]
pub struct History {
    past: Vec<LedgerEntry>,
    future: Vec<LedgerEntry>,
}

impl History {
    /// An empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Commit a new value for `resource` through `transport`, recording the
    /// forward and inverse operations. Clears the redo stack.
    pub fn commit<T: Transport>(
        &mut self,
        transport: &mut T,
        resource: &Symbol,
        new_value: Expr,
    ) -> Result<()> {
        let old_value = transport.read(resource)?;
        transport.realize(resource, &set_value_op(new_value.clone()))?;
        self.past.push(LedgerEntry {
            resource: resource.clone(),
            forward: set_value_op(new_value),
            inverse: set_value_op(old_value),
        });
        self.future.clear();
        Ok(())
    }

    /// Undo the most recent edit by replaying its inverse operation. Returns the
    /// resource that changed, or `None` if there is nothing to undo.
    pub fn undo<T: Transport>(&mut self, transport: &mut T) -> Result<Option<Symbol>> {
        let Some(entry) = self.past.pop() else {
            return Ok(None);
        };
        transport.realize(&entry.resource, &entry.inverse)?;
        let resource = entry.resource.clone();
        self.future.push(entry);
        Ok(Some(resource))
    }

    /// Redo the most recently undone edit by replaying its forward operation.
    pub fn redo<T: Transport>(&mut self, transport: &mut T) -> Result<Option<Symbol>> {
        let Some(entry) = self.future.pop() else {
            return Ok(None);
        };
        transport.realize(&entry.resource, &entry.forward)?;
        let resource = entry.resource.clone();
        self.past.push(entry);
        Ok(Some(resource))
    }

    /// Whether there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    /// Whether there is an edit to redo.
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// The ledger as a value: the ordered list of recorded operations (the
    /// object edit history / session event log).
    pub fn as_value(&self) -> Expr {
        Expr::List(
            self.past
                .iter()
                .map(|entry| {
                    Expr::Map(vec![
                        (
                            Expr::Symbol(Symbol::new("resource")),
                            Expr::Symbol(entry.resource.clone()),
                        ),
                        (Expr::Symbol(Symbol::new("op")), entry.forward.clone()),
                        (Expr::Symbol(Symbol::new("inverse")), entry.inverse.clone()),
                    ])
                })
                .collect(),
        )
    }
}

/// Named snapshots of values (workspaces or objects), kept as data.
#[derive(Default)]
pub struct Snapshots {
    named: BTreeMap<String, Expr>,
    order: Vec<String>,
}

impl Snapshots {
    /// An empty snapshot store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Take (or replace) a named snapshot of `value`.
    pub fn take(&mut self, name: &str, value: Expr) {
        if !self.named.contains_key(name) {
            self.order.push(name.to_owned());
        }
        self.named.insert(name.to_owned(), value);
    }

    /// Restore a named snapshot.
    pub fn restore(&self, name: &str) -> Option<Expr> {
        self.named.get(name).cloned()
    }

    /// The snapshot names in creation order.
    pub fn names(&self) -> &[String] {
        &self.order
    }
}

/// Append-only session event log, kept as a value.
#[derive(Default)]
pub struct SessionLog {
    events: Vec<Expr>,
}

impl SessionLog {
    /// An empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an event value.
    pub fn append(&mut self, event: Expr) {
        self.events.push(event);
    }

    /// The log as a value.
    pub fn as_value(&self) -> Expr {
        Expr::List(self.events.clone())
    }

    /// The number of logged events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Attach a review comment to an object, returning the object with an appended
/// annotation. Annotations are object-review primitives kept as data.
pub fn annotate(object: &Expr, author: &str, comment: &str) -> Result<Expr> {
    let Expr::Map(entries) = object else {
        return Err(Error::HostError(
            "annotations attach to map-shaped objects".to_owned(),
        ));
    };
    let mut entries = entries.clone();
    let annotation = Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("author")),
            Expr::Symbol(Symbol::new(author)),
        ),
        (
            Expr::Symbol(Symbol::new("text")),
            Expr::String(comment.to_owned()),
        ),
    ]);
    let key = Expr::Symbol(Symbol::new("annotations"));
    if let Some(slot) = entries.iter_mut().find(|(entry_key, _)| entry_key == &key) {
        if let Expr::List(list) = &mut slot.1 {
            list.push(annotation);
        } else {
            slot.1 = Expr::List(vec![annotation]);
        }
    } else {
        entries.push((key, Expr::List(vec![annotation])));
    }
    Ok(Expr::Map(entries))
}
