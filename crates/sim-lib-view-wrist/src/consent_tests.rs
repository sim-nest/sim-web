use std::sync::Arc;

use sim_kernel::{CapabilitySet, Cx, DefaultFactory, EagerPolicy, Error, Expr};
use sim_lib_view_device::{
    ConsentReceipt, DeviceSampleStore, EdgeId, FrameClock, RateClass, StoreKey,
};
use sim_value::{access, build};

use crate::{
    WatchCapability, WatchCommand, active_watch_consent_badge_cluster, ingest_worn_expr,
    store_worn_sample, sweep_watch_privacy, watch_health_grant, watch_location_grant,
    watch_mic_grant, worn_event_capability,
};

#[test]
fn watch_capability_names_are_default_denied() {
    let cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    assert_eq!(WatchCapability::Health.as_str(), "watch/health");
    assert_eq!(WatchCapability::Location.as_str(), "watch/location");
    assert_eq!(WatchCapability::Mic.as_str(), "watch/mic");
    assert_eq!(
        WatchCapability::VendorReport.as_str(),
        "watch/vendor-report"
    );
    assert_eq!(
        WatchCapability::from_name("watch/vendor-report"),
        Some(WatchCapability::VendorReport)
    );

    for capability in WatchCapability::ALL {
        assert!(matches!(
            cx.require(&capability.capability_name()),
            Err(Error::CapabilityDenied { .. })
        ));
    }
}

#[test]
fn watch_worn_ingest_is_grant_and_session_bound() {
    let health = worn_event("heart-rate", 1);
    let location = worn_event("gps", 2);
    let mic = worn_event("mic-audio", 3);
    assert_eq!(
        worn_event_capability(&health).unwrap(),
        WatchCapability::Health
    );
    assert_eq!(
        worn_event_capability(&location).unwrap(),
        WatchCapability::Location
    );
    assert_eq!(worn_event_capability(&mic).unwrap(), WatchCapability::Mic);

    let session = EdgeId::named("trex");
    let other_session = EdgeId::named("other");
    let receipt = ConsentReceipt::new(
        vec![
            watch_health_grant(),
            watch_location_grant(),
            watch_mic_grant(),
        ],
        1_000,
        Vec::new(),
        session.clone(),
        11,
    );
    let cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    for event in [&health, &location, &mic] {
        assert!(matches!(
            ingest_worn_expr(&cx, event, &receipt, &session),
            Err(Error::CapabilityDenied { .. })
        ));
    }

    let granted = CapabilitySet::new()
        .grant(WatchCapability::Health.capability_name())
        .grant(WatchCapability::Location.capability_name())
        .grant(WatchCapability::Mic.capability_name());
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.with_capabilities(granted, |cx| {
        assert_eq!(
            ingest_worn_expr(cx, &health, &receipt, &session).unwrap(),
            health
        );
        assert_eq!(
            ingest_worn_expr(cx, &location, &receipt, &session).unwrap(),
            location
        );
        assert_eq!(ingest_worn_expr(cx, &mic, &receipt, &session).unwrap(), mic);

        let missing_visible =
            ConsentReceipt::new(Vec::new(), 1_000, Vec::new(), session.clone(), 12);
        assert!(matches!(
            ingest_worn_expr(cx, &health, &missing_visible, &session),
            Err(Error::HostError(message)) if message.contains("visible consent")
        ));

        assert!(matches!(
            ingest_worn_expr(cx, &health, &receipt, &other_session),
            Err(Error::HostError(message)) if message.contains("not for this session")
        ));
        Ok(())
    })
    .unwrap();
}

#[test]
fn watch_privacy_reaper_evicts_sensitive_samples_on_modeled_clock() {
    let session = EdgeId::named("trex");
    let command = WatchCommand::PrivacyMode {
        enabled: true,
        window_ms: 1_000,
    };
    let receipt = command
        .privacy_consent_receipt(session, 21)
        .expect("privacy command creates receipt");

    let cluster = active_watch_consent_badge_cluster(std::slice::from_ref(&receipt));
    sim_lib_scene::validate_scene(&cluster).unwrap();
    assert_eq!(
        access::field_sym(&cluster, "kind")
            .expect("kind")
            .as_qualified_str(),
        "scene/badge-cluster"
    );

    let mut store = DeviceSampleStore::new();
    let health_content = StoreKey::named("watch/hr/content");
    let location_content = StoreKey::named("watch/gps/content");
    store.insert_content(health_content.clone(), build::text("heart-rate raw"));
    store.insert_content(location_content.clone(), build::text("gps raw"));

    let health_key = store_worn_sample(
        &mut store,
        &worn_event("heart-rate", 4),
        &receipt,
        FrameClock::new(0, RateClass::watch()),
        vec![health_content.clone()],
    )
    .unwrap();
    let location_key = store_worn_sample(
        &mut store,
        &worn_event("gps", 5),
        &receipt,
        FrameClock::new(0, RateClass::watch()),
        vec![location_content.clone()],
    )
    .unwrap();

    let evicted = sweep_watch_privacy(
        &mut store,
        std::slice::from_ref(&receipt),
        FrameClock::new(0, RateClass::watch()),
    );
    assert!(evicted.is_empty());
    assert_eq!(
        access::field(store.sample(&health_key).unwrap().value(), "health"),
        Some(&build::qsym("device/reaper", "redacted"))
    );
    assert_eq!(
        access::field(store.sample(&location_key).unwrap().value(), "location"),
        Some(&build::qsym("device/reaper", "redacted"))
    );

    let evicted = sweep_watch_privacy(
        &mut store,
        &[receipt],
        FrameClock::new(2, RateClass::watch()),
    );
    assert!(evicted.iter().any(|item| item.key == health_key));
    assert!(evicted.iter().any(|item| item.key == location_key));
    assert!(evicted.iter().any(|item| item.key == health_content));
    assert!(evicted.iter().any(|item| item.key == location_content));
    assert!(!store.contains_sample(&health_key));
    assert!(!store.contains_sample(&location_key));
    assert!(!store.contains_content(&health_content));
    assert!(!store.contains_content(&location_content));
}

fn worn_event(sensor: &str, seq: u64) -> Expr {
    build::map(vec![
        ("kind", build::qsym("stream/device-sample", "record")),
        ("sample", build::qsym("stream/device-sample", "worn-event")),
        ("seq", build::uint(seq)),
        ("sensor", build::qsym("stream/worn-sensor", sensor)),
        ("confidence", build::uint(9_500)),
        (
            "payload",
            build::map(vec![("kind", build::qsym("watch/test", sensor))]),
        ),
    ])
}
