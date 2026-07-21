use std::sync::Arc;

use sim_kernel::{CapabilitySet, Cx, DefaultFactory, EagerPolicy, Error, Expr, Result, Symbol};
use sim_lib_view_device::{
    ConsentReceipt, DeviceSampleStore, EdgeId, FrameClock, RateClass, StoreKey,
};
use sim_value::{access, build};

use crate::{
    GlassesCapability, active_glasses_consent_badge_cluster, glasses_camera_grant,
    glasses_capability_for_expr, glasses_hand_grant, glasses_mic_grant, glasses_pose_grant,
    glasses_vendor_report_grant, glasses_world_anchor_grant, halo_consent_glyph,
    require_glasses_expr_consent, store_glasses_sample, sweep_glasses_privacy,
};

#[test]
fn glasses_consent_session_bound_and_default_denied() {
    let samples = [
        (GlassesCapability::Pose, xr_sample("pose")),
        (GlassesCapability::Camera, xr_sample("camera-frame")),
        (GlassesCapability::WorldAnchor, world_anchor()),
        (GlassesCapability::Hand, xr_sample("hand")),
        (GlassesCapability::Mic, xr_sample("mic-chunk")),
        (GlassesCapability::VendorReport, vendor_report()),
    ];
    for (capability, sample) in &samples {
        assert_eq!(glasses_capability_for_expr(sample).unwrap(), *capability);
    }
    assert_eq!(
        GlassesCapability::from_name("glasses/vendor-report"),
        Some(GlassesCapability::VendorReport)
    );

    let session = EdgeId::named("viture-halo");
    let other_session = EdgeId::named("other");
    let receipt = all_glasses_receipt(session.clone(), 11, 1_000);
    let cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    for capability in GlassesCapability::ALL {
        assert!(matches!(
            cx.require(&capability.capability_name()),
            Err(Error::CapabilityDenied { .. })
        ));
    }
    for (_, sample) in &samples {
        assert!(matches!(
            require_glasses_expr_consent(&cx, sample, &receipt, &session),
            Err(Error::CapabilityDenied { .. })
        ));
    }

    let granted = GlassesCapability::ALL
        .into_iter()
        .fold(CapabilitySet::new(), |set, capability| {
            set.grant(capability.capability_name())
        });
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.with_capabilities(granted, |cx| -> Result<()> {
        let missing_visible =
            ConsentReceipt::new(Vec::new(), 1_000, Vec::new(), session.clone(), 12);
        assert!(matches!(
            require_glasses_expr_consent(cx, &samples[1].1, &missing_visible, &session),
            Err(Error::HostError(message)) if message.contains("visible consent")
        ));
        assert!(matches!(
            require_glasses_expr_consent(cx, &samples[1].1, &receipt, &other_session),
            Err(Error::HostError(message)) if message.contains("not for this session")
        ));
        for (capability, sample) in &samples {
            assert_eq!(
                require_glasses_expr_consent(cx, sample, &receipt, &session)?,
                *capability
            );
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn glasses_reaper_evicts_camera_and_mic_refs_on_modeled_clock() {
    let session = EdgeId::named("viture-halo");
    let receipt = ConsentReceipt::new(
        vec![glasses_camera_grant(), glasses_mic_grant()],
        1_000,
        vec![Symbol::new("camera"), Symbol::new("mic")],
        session,
        42,
    );
    let cluster = active_glasses_consent_badge_cluster(std::slice::from_ref(&receipt));
    sim_lib_scene::validate_scene(&cluster).unwrap();
    assert_eq!(
        access::field_sym(&cluster, "kind")
            .expect("badge kind")
            .as_qualified_str(),
        "scene/badge-cluster"
    );
    let halo = halo_consent_glyph(std::slice::from_ref(&receipt));
    sim_lib_scene::validate_scene(&halo).unwrap();
    assert_eq!(
        access::field_sym(&halo, "kind")
            .expect("glance kind")
            .as_qualified_str(),
        "scene/glance"
    );

    let camera_content = StoreKey::named("glasses/camera/content");
    let mic_content = StoreKey::named("glasses/mic/content");
    let mut store = DeviceSampleStore::new();
    store.insert_content(camera_content.clone(), build::text("camera bytes"));
    store.insert_content(mic_content.clone(), build::text("mic bytes"));

    let camera_key = store_glasses_sample(
        &mut store,
        GlassesCapability::Camera,
        "camera-1",
        build::text("camera ref"),
        &receipt,
        FrameClock::new(0, RateClass::safe_default()),
        vec![camera_content.clone()],
    )
    .unwrap();
    let mic_key = store_glasses_sample(
        &mut store,
        GlassesCapability::Mic,
        "mic-1",
        build::text("mic ref"),
        &receipt,
        FrameClock::new(0, RateClass::safe_default()),
        vec![mic_content.clone()],
    )
    .unwrap();

    let evicted = sweep_glasses_privacy(
        &mut store,
        std::slice::from_ref(&receipt),
        FrameClock::new(0, RateClass::safe_default()),
    );
    assert!(evicted.is_empty());
    assert_eq!(
        access::field(store.sample(&camera_key).unwrap().value(), "camera"),
        Some(&build::qsym("device/reaper", "redacted"))
    );
    assert_eq!(
        access::field(store.sample(&mic_key).unwrap().value(), "mic"),
        Some(&build::qsym("device/reaper", "redacted"))
    );

    let evicted = sweep_glasses_privacy(
        &mut store,
        &[receipt],
        FrameClock::new(2, RateClass::safe_default()),
    );
    assert!(evicted.iter().any(|item| item.key == camera_key));
    assert!(evicted.iter().any(|item| item.key == mic_key));
    assert!(evicted.iter().any(|item| item.key == camera_content));
    assert!(evicted.iter().any(|item| item.key == mic_content));
    assert!(!store.contains_sample(&camera_key));
    assert!(!store.contains_sample(&mic_key));
    assert!(!store.contains_content(&camera_content));
    assert!(!store.contains_content(&mic_content));
}

fn all_glasses_receipt(session: EdgeId, seq: u64, retain_ms: u64) -> ConsentReceipt {
    ConsentReceipt::new(
        vec![
            glasses_pose_grant(),
            glasses_camera_grant(),
            glasses_world_anchor_grant(),
            glasses_hand_grant(),
            glasses_mic_grant(),
            glasses_vendor_report_grant(),
        ],
        retain_ms,
        Vec::new(),
        session,
        seq,
    )
}

fn xr_sample(kind: &str) -> Expr {
    build::map(vec![
        ("kind", build::qsym("stream/device-sample", "record")),
        ("sample", build::qsym("xr", kind)),
    ])
}

fn world_anchor() -> Expr {
    build::map(vec![
        ("kind", build::qsym("workspace", "panel-placement")),
        ("world-anchor", build::qsym("glasses/world-anchor", "desk")),
    ])
}

fn vendor_report() -> Expr {
    build::map(vec![("kind", build::qsym("glasses", "vendor-report"))])
}
