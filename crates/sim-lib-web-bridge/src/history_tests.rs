//! Tests for history, snapshots, the session log, and annotations.

use sim_kernel::{Expr, NumberLiteral};

use crate::fixture::FixtureTransport;
use crate::history::{History, SessionLog, Snapshots, annotate};
use crate::transport::Transport;

use sim_value::build::keyword as sym;

fn number(value: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: sym("i64"),
        canonical: value.to_owned(),
    })
}

#[test]
fn undo_and_redo_replay_real_inverse_operations() {
    let v0 = number("0");
    let v1 = number("1");
    let v2 = number("2");
    let mut transport = FixtureTransport::new().with(sym("doc"), v0.clone());
    let mut history = History::new();

    history
        .commit(&mut transport, &sym("doc"), v1.clone())
        .unwrap();
    history
        .commit(&mut transport, &sym("doc"), v2.clone())
        .unwrap();
    assert_eq!(transport.read(&sym("doc")).unwrap(), v2);

    // Undo replays inverse operations, restoring prior states exactly.
    assert_eq!(history.undo(&mut transport).unwrap(), Some(sym("doc")));
    assert_eq!(transport.read(&sym("doc")).unwrap(), v1);
    history.undo(&mut transport).unwrap();
    assert_eq!(transport.read(&sym("doc")).unwrap(), v0);
    assert!(!history.can_undo());

    // Redo replays the forward operations.
    history.redo(&mut transport).unwrap();
    assert_eq!(transport.read(&sym("doc")).unwrap(), v1);
    assert!(history.can_redo());

    // A fresh commit clears the redo stack.
    history
        .commit(&mut transport, &sym("doc"), number("9"))
        .unwrap();
    assert!(!history.can_redo());
    // The ledger is a value.
    assert!(matches!(history.as_value(), Expr::List(_)));
}

#[test]
fn named_snapshots_restore_prior_states_exactly() {
    let mut snapshots = Snapshots::new();
    let a = Expr::Map(vec![(Expr::Symbol(sym("v")), number("1"))]);
    let b = Expr::Map(vec![(Expr::Symbol(sym("v")), number("2"))]);
    snapshots.take("before", a.clone());
    snapshots.take("after", b.clone());
    assert_eq!(snapshots.restore("before"), Some(a));
    assert_eq!(snapshots.restore("after"), Some(b));
    assert_eq!(
        snapshots.names(),
        &["before".to_owned(), "after".to_owned()]
    );
    assert_eq!(snapshots.restore("missing"), None);
}

#[test]
fn the_session_log_is_an_inspectable_value() {
    let mut log = SessionLog::new();
    assert!(log.is_empty());
    log.append(Expr::Symbol(sym("opened")));
    log.append(Expr::Symbol(sym("edited")));
    assert_eq!(log.len(), 2);
    let Expr::List(events) = log.as_value() else {
        panic!("the log is a list value")
    };
    assert_eq!(events.len(), 2);
}

#[test]
fn annotations_attach_review_comments_as_data() {
    let object = Expr::Map(vec![(
        Expr::Symbol(sym("title")),
        Expr::String("draft".to_owned()),
    )]);
    let reviewed = annotate(&object, "bo", "needs a citation").unwrap();
    let reviewed = annotate(&reviewed, "ada", "and a figure").unwrap();
    let Expr::Map(entries) = &reviewed else {
        panic!("annotated object is a map")
    };
    let annotations = entries
        .iter()
        .find(|(k, _)| matches!(k, Expr::Symbol(s) if &*s.name == "annotations"))
        .map(|(_, v)| v);
    assert!(matches!(annotations, Some(Expr::List(list)) if list.len() == 2));
}
