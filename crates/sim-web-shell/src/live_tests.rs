use super::*;
use sim_lib_intent::{Origin, intent};
use std::{collections::BTreeMap, sync::Arc};

fn key_path(key: &str) -> Expr {
    Expr::List(vec![Expr::Vector(vec![
        Expr::Symbol(Symbol::new("k")),
        Expr::Symbol(Symbol::new(key)),
    ])])
}

fn edit_intent(key: &str, value: &str) -> Expr {
    intent(
        "edit-field",
        Origin::human(1),
        vec![
            ("target", demo_value()),
            ("path", key_path(key)),
            ("value", Expr::String(value.to_owned())),
        ],
    )
}

#[test]
fn submit_edit_returns_a_patch_that_reconstructs_the_scene() {
    let mut live = LiveSession::new(sim_kernel::HandleSeed::new(0x5745_4260)).unwrap();
    let before = live.open(DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
    sim_lib_scene::validate_scene(&before).expect("initial scene is valid");

    let updates = live
        .submit(DEFAULT_PANE, &edit_intent("title", "changed"))
        .unwrap();
    assert_eq!(updates.len(), 1, "the subscribed pane updates exactly once");
    let update = &updates[0];
    assert_ne!(update.scene, before, "the Scene changed");
    let rebuilt = sim_lib_scene::apply(&before, &update.diff).unwrap();
    assert_eq!(
        rebuilt, update.scene,
        "the diff reconstructs the new Scene from the old one"
    );
}

#[test]
fn open_returns_a_valid_scene() {
    let mut live = LiveSession::new(sim_kernel::HandleSeed::new(0x5745_4261)).unwrap();
    let scene = live.open(DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
    sim_lib_scene::validate_scene(&scene).expect("open returns a valid Scene");
}

#[test]
fn a_browser_json_intent_decodes_and_drives_a_root_edit() {
    let body = r#"{"kind":"intent/edit-field","origin":{"operator":"human","at-tick":2},"target":{},"path":[],"value":"hello"}"#;
    let intent = decode_intent_body(body).unwrap();
    let kind = match &intent {
        Expr::Map(entries) => entries.iter().find_map(|(key, value)| {
            matches!(key, Expr::Symbol(symbol) if &*symbol.name == "kind").then_some(value)
        }),
        _ => None,
    };
    assert!(
        matches!(kind, Some(Expr::Symbol(_))),
        "the kind tag is lifted to a symbol"
    );

    let mut live = LiveSession::new(sim_kernel::HandleSeed::new(0x5745_4262)).unwrap();
    live.open(DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
    let updates = live.submit(DEFAULT_PANE, &intent).unwrap();
    assert_eq!(updates.len(), 1);
}

#[test]
fn a_malformed_body_is_an_error_not_a_panic() {
    assert!(decode_intent_body("this is not json").is_err());
    assert!(
        decode_intent_body("[1, 2, 3]").is_err(),
        "a non-object intent body is rejected"
    );
}

#[test]
fn an_intent_without_a_kind_fails_closed_on_submit() {
    let intent = decode_intent_body(r#"{"origin":{"operator":"human","at-tick":1}}"#).unwrap();
    let mut live = LiveSession::new(sim_kernel::HandleSeed::new(0x5745_4263)).unwrap();
    assert!(
        live.submit(DEFAULT_PANE, &intent).is_err(),
        "an intent without a kind is rejected, not executed"
    );
}

#[test]
fn patches_scenes_and_errors_encode_as_untagged_json() {
    let mut live = LiveSession::new(sim_kernel::HandleSeed::new(0x5745_4264)).unwrap();
    live.open(DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
    let updates = live
        .submit(DEFAULT_PANE, &edit_intent("title", "x"))
        .unwrap();

    let patches = encode_patches(&updates);
    assert!(patches.contains("\"patches\""), "carries a patches array");
    assert!(patches.contains("scene/patch"), "patches are scene patches");

    let scene = encode_scene(&live.open(DEFAULT_RESOURCE, DEFAULT_PANE).unwrap());
    assert!(scene.contains("\"scene\""), "carries a scene field");

    assert!(
        error_json("boom").contains("boom"),
        "errors carry a message"
    );
}

#[test]
fn session_ids_are_opaque_and_validated() {
    let mut table = LiveSessionTable::with_config(
        Box::new(DefaultLiveSurfaceFactory::new(sim_kernel::HandleSeed::new(
            0x5745_4280,
        ))),
        LiveSessionTableConfig {
            capacity: 2,
            idle_ttl: Duration::from_secs(60),
        },
    );
    let (session_id, _) = table.open(None, DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
    assert_eq!(session_id.len(), 32);
    assert!(session_id.bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(
        table
            .submit("not-a-session", DEFAULT_PANE, &edit_intent("title", "x"))
            .is_err(),
        "malformed ids fail closed"
    );
}

#[test]
fn browser_sessions_are_isolated() {
    let mut table = LiveSessionTable::new(Box::new(DefaultLiveSurfaceFactory::new(
        sim_kernel::HandleSeed::new(0x5745_4281),
    )));
    let (left, _) = table.open(None, DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
    let (right, right_before) = table.open(None, DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
    table
        .submit(&left, DEFAULT_PANE, &edit_intent("title", "left-only"))
        .unwrap();
    let (_, right_after) = table
        .open(Some(&right), DEFAULT_RESOURCE, DEFAULT_PANE)
        .unwrap();
    assert_eq!(
        right_after, right_before,
        "a write in one browser session must not alter another session"
    );
}

#[test]
fn close_cancels_a_session() {
    let mut table = LiveSessionTable::new(Box::new(DefaultLiveSurfaceFactory::new(
        sim_kernel::HandleSeed::new(0x5745_4282),
    )));
    let (session_id, _) = table.open(None, DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
    table.close(&session_id).unwrap();
    assert!(
        table
            .submit(&session_id, DEFAULT_PANE, &edit_intent("title", "closed"))
            .is_err(),
        "closed sessions cannot be reused"
    );
}

#[test]
fn idle_sessions_expire() {
    let start = Duration::ZERO;
    let mut table = LiveSessionTable::with_config(
        Box::new(DefaultLiveSurfaceFactory::new(sim_kernel::HandleSeed::new(
            0x5745_4283,
        ))),
        LiveSessionTableConfig {
            capacity: 4,
            idle_ttl: Duration::from_secs(1),
        },
    );
    let (session_id, _) = table
        .open_at(None, DEFAULT_RESOURCE, DEFAULT_PANE, start)
        .unwrap();
    let later = start + Duration::from_secs(2);
    assert!(
        table
            .submit_at(
                &session_id,
                DEFAULT_PANE,
                &edit_intent("title", "expired"),
                later,
            )
            .is_err(),
        "idle sessions are evicted before use"
    );
    assert_eq!(table.len(), 0);
}

#[test]
fn capacity_evicts_the_oldest_session_deterministically() {
    let start = Duration::ZERO;
    let mut table = LiveSessionTable::with_config(
        Box::new(DefaultLiveSurfaceFactory::new(sim_kernel::HandleSeed::new(
            0x5745_4284,
        ))),
        LiveSessionTableConfig {
            capacity: 2,
            idle_ttl: Duration::from_secs(60),
        },
    );
    let (first, _) = table
        .open_at(None, DEFAULT_RESOURCE, DEFAULT_PANE, start)
        .unwrap();
    let (second, _) = table
        .open_at(
            None,
            DEFAULT_RESOURCE,
            DEFAULT_PANE,
            start + Duration::from_secs(1),
        )
        .unwrap();
    table
        .submit_at(
            &second,
            DEFAULT_PANE,
            &edit_intent("title", "touch"),
            start + Duration::from_secs(2),
        )
        .unwrap();
    let (third, _) = table
        .open_at(
            None,
            DEFAULT_RESOURCE,
            DEFAULT_PANE,
            start + Duration::from_secs(3),
        )
        .unwrap();
    assert_ne!(third, first);
    assert_eq!(table.len(), 2);
    assert!(
        table
            .submit_at(
                &first,
                DEFAULT_PANE,
                &edit_intent("title", "evicted"),
                start + Duration::from_secs(4),
            )
            .is_err(),
        "the oldest idle session is evicted first"
    );
    table
        .submit_at(
            &second,
            DEFAULT_PANE,
            &edit_intent("title", "survives"),
            start + Duration::from_secs(5),
        )
        .unwrap();
}

#[test]
fn modeled_clock_and_entropy_drive_session_lifecycle() {
    let transport = Arc::new(sim_transport_ports::model::ScriptedStreamPort::new([]));
    let services = Arc::new(crate::ModelShellServices::new(
        transport.services(),
        BTreeMap::new(),
    ));
    let mut table = LiveSessionTable::with_config_and_services(
        Box::new(DefaultLiveSurfaceFactory::new(sim_kernel::HandleSeed::new(
            0x5745_4285,
        ))),
        LiveSessionTableConfig {
            capacity: 2,
            idle_ttl: Duration::from_secs(1),
        },
        services.clone(),
    );
    let (first, _) = table.open(None, DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
    assert_eq!(first, "01000000000000000000000000000000");
    services.set_time(Duration::from_secs(2));
    assert!(
        table
            .submit(&first, DEFAULT_PANE, &edit_intent("title", "stale"))
            .is_err()
    );
    let (second, _) = table.open(None, DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
    assert_eq!(second, "02000000000000000000000000000000");
}
