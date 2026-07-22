use std::sync::Arc;

use sim_kernel::{CapabilitySet, Cx, DefaultFactory, EagerPolicy, Error, EventLedger, Ref, Symbol};
use sim_value::build;

use crate::{
    ConsentReceipt, DeviceCapability, DeviceSampleStore, EdgeId, FrameClock, RateClass,
    RetentionReaper, StoreKey, StoredSample, record_consent_receipt, require_with_consent,
};

#[test]
fn device_consent_session_bound_and_reaper_evicts() {
    let session = EdgeId::named("primary");
    let other_session = EdgeId::named("secondary");
    let mut ledger = EventLedger::new();
    let receipt = record_consent_receipt(
        &mut ledger,
        Ref::Symbol(Symbol::qualified("device/run", "one")),
        vec![DeviceCapability::Pose.grant_symbol()],
        500,
        vec![Symbol::qualified("device/redact", "raw-frame")],
        session.clone(),
    )
    .unwrap();
    assert_eq!(receipt.seq, 0);
    assert_eq!(ledger.len(), 1);
    assert_eq!(
        ConsentReceipt::from_expr(&receipt.to_expr()).unwrap(),
        receipt
    );
    sim_lib_scene::validate_scene(&receipt.to_badge_scene()).unwrap();
    sim_lib_scene::validate_scene(&receipt.to_glance_scene()).unwrap();

    let cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    assert!(matches!(
        require_with_consent(&cx, DeviceCapability::Pose.as_str(), &receipt, &session),
        Err(Error::CapabilityDenied { .. })
    ));

    let granted = CapabilitySet::new().grant(DeviceCapability::Pose.capability_name());
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.with_capabilities(granted, |cx| {
        let missing_grant = ConsentReceipt::new(Vec::new(), 500, Vec::new(), session.clone(), 7);
        assert!(matches!(
            require_with_consent(cx, DeviceCapability::Pose.as_str(), &missing_grant, &session),
            Err(Error::HostError(message)) if message.contains("visible consent")
        ));
        assert!(matches!(
            require_with_consent(cx, DeviceCapability::Pose.as_str(), &receipt, &other_session),
            Err(Error::HostError(message)) if message.contains("not for this session")
        ));
        require_with_consent(cx, DeviceCapability::Pose.as_str(), &receipt, &session)
    })
    .unwrap();

    let content_key = StoreKey::named("pose/content");
    let sample_key = StoreKey::named("pose/sample");
    let mut store = DeviceSampleStore::new();
    store.insert_content(content_key.clone(), build::text("pose bytes"));
    store.insert_sample(StoredSample::new(
        sample_key.clone(),
        receipt.seq,
        0,
        vec![content_key.clone()],
        build::map(vec![
            ("raw-frame", build::text("sensitive")),
            ("pose", build::qsym("device/sample", "pose")),
        ]),
    ));

    RetentionReaper::new().sweep(
        &mut store,
        std::slice::from_ref(&receipt),
        FrameClock::new(0, RateClass::safe_default()),
    );
    let sample = store.sample(&sample_key).unwrap();
    let sim_kernel::Expr::Map(fields) = sample.value() else {
        panic!("sample value remains a map");
    };
    assert!(fields.iter().any(|(key, value)| {
        key == &build::sym("raw-frame") && value == &build::qsym("device/reaper", "redacted")
    }));

    let evicted = RetentionReaper::new().sweep(
        &mut store,
        &[receipt],
        FrameClock::new(1, RateClass::safe_default()),
    );

    assert!(!store.contains_sample(&sample_key));
    assert!(!store.contains_content(&content_key));
    assert!(evicted.iter().any(|item| item.key == sample_key));
    assert!(evicted.iter().any(|item| item.key == content_key));
}
