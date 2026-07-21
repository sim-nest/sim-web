use sim_kernel::{Expr, Symbol};
use sim_lib_view::SurfaceCaps;
use sim_value::access;

use crate::{
    DegradationResolver, DeviceProfile, DeviceSurfaceCapsExt, DeviceTier, GlassesClass,
    ObservedRoute, RateClass, T_REX_3_PRO_48_CAPS_FIXTURE, WORN_CAPS_KIND, WORN_CAPS_NAMESPACE,
    derive_tier, glasses_class, tier_preset, trex3pro_48_worn_caps_fixture, worn_caps_fixture,
    worn_caps_fixture_names,
};

#[test]
fn presets_at_each_tier_round_trip_through_expr() {
    for tier in DeviceTier::ALL {
        let profile = tier_preset(tier);
        assert_eq!(profile.tier, tier);
        let back = DeviceProfile::from_expr(&profile.to_expr()).expect("profile round-trips");
        assert_eq!(back, profile);
    }
}

#[test]
fn derive_tier_is_authoritative_and_rate_defaults_safe() {
    for tier in DeviceTier::ALL {
        let profile = tier_preset(tier);
        assert_eq!(derive_tier(&profile), tier);
        for lower in DeviceTier::ALL {
            assert_eq!(tier.supports(lower), tier >= lower);
        }
    }
}

#[test]
fn surface_caps_extension_derives_rate_and_tier() {
    let caps = SurfaceCaps::from_preset("watch", "watch.local.1").expect("watch caps");
    let profile = caps.device_profile();
    assert_eq!(profile.kind.name.as_ref(), "watch");
    assert_eq!(profile.rate, RateClass::watch());
    assert_eq!(profile.tier, DeviceTier::Actuator);
}

#[test]
fn watch_presets_are_device_tiers_with_rate() {
    let presets = [
        "watch",
        "watch-glance",
        "watch-glance-large",
        "watch-sport",
        "watch-sleep",
    ];
    for preset in presets {
        let caps = SurfaceCaps::from_preset(preset, format!("{preset}.local"))
            .expect("watch preset exists");
        let back = SurfaceCaps::from_expr(&caps.to_expr()).expect("surface caps round-trip");
        assert_eq!(back, caps);

        let profile = caps.device_profile();
        assert_eq!(derive_tier(&profile), profile.tier, "{preset}");
        assert!(
            matches!(profile.tier, DeviceTier::Sensor | DeviceTier::Actuator),
            "{preset} resolved to {:?}",
            profile.tier
        );
        assert_ne!(profile.tier, DeviceTier::Rich, "{preset} must not be rich");
        assert_eq!(profile.rate, RateClass::watch(), "{preset}");
        assert!(has_symbol(&profile.display, "round"), "{preset}");
        assert!(has_symbol(&profile.links, "phone-relay"), "{preset}");
        assert!(has_symbol(&profile.output, "screen"), "{preset}");
        assert!(has_symbol(&profile.streams, "battery"), "{preset}");
        assert!(!has_symbol(&profile.input, "voice"), "{preset}");
    }

    let large = SurfaceCaps::from_preset("watch-glance-large", "watch.local.48")
        .expect("large watch preset");
    let large_profile = large.device_profile();
    assert!(has_symbol(&large_profile.input, "mic"));
    assert!(has_symbol(&large_profile.output, "haptic"));
    assert!(has_symbol(&large_profile.output, "face"));
    assert!(has_symbol(&large_profile.output, "speaker"));
    assert!(has_symbol(&large_profile.output, "mic"));
    assert!(access::field(&large_profile.to_expr(), "asr-site").is_none());
    assert!(!has_symbol(&large_profile.input, "crown"));

    let generic = SurfaceCaps::from_preset("watch", "watch.local.generic").expect("watch preset");
    assert!(has_symbol(&generic.device_profile().input, "crown"));
}

#[test]
fn weaker_watch_preset_is_a_subset() {
    let sport = SurfaceCaps::from_preset("watch-sport", "watch.local.sport")
        .expect("sport watch preset")
        .device_profile();
    let sleep = SurfaceCaps::from_preset("watch-sleep", "watch.local.sleep")
        .expect("sleep watch preset")
        .device_profile();

    assert_symbol_subset(&sleep.input, &sport.input);
    assert_symbol_subset(&sleep.output, &sport.output);
    assert_symbol_subset(&sleep.streams, &sport.streams);
    assert_symbol_subset(&sleep.links, &sport.links);
}

#[test]
fn glasses_presets_span_the_ladder_with_rateclass() {
    let presets = [
        "glasses",
        "glasses-hud",
        "glasses-hud-camera",
        "glasses-3dof",
        "glasses-stereo",
        "glasses-luma-ultra",
    ];
    for preset in presets {
        let caps = SurfaceCaps::from_preset(preset, format!("{preset}.local"))
            .expect("glasses preset exists");
        let back = SurfaceCaps::from_expr(&caps.to_expr()).expect("surface caps round-trip");
        assert_eq!(back, caps, "{preset}");

        let profile = caps.device_profile();
        assert_eq!(derive_tier(&profile), profile.tier, "{preset}");
        assert!(
            glasses_class(&profile).is_some(),
            "{preset} must resolve to a glasses profile"
        );
        assert!(has_symbol(&profile.display, "glance"), "{preset}");
    }

    let display_only = SurfaceCaps::from_preset("glasses", "glasses.local.display")
        .expect("display-only glasses")
        .device_profile();
    assert_eq!(display_only.tier, DeviceTier::Display);
    assert_eq!(display_only.rate, RateClass::safe_default());
    assert_eq!(
        glasses_class(&display_only),
        Some(GlassesClass::DisplayOnly)
    );
    assert!(has_symbol(&display_only.display, "mono"));
    assert!(has_symbol(&display_only.links, "usb"));

    let halo = SurfaceCaps::from_preset("glasses-hud", "halo.local.hud")
        .expect("halo hud")
        .device_profile();
    assert_eq!(halo.tier, DeviceTier::Actuator);
    assert_eq!(halo.rate.content_hz, 5);
    assert_eq!(halo.rate.adapt_hz, 30);
    assert_eq!(halo.rate.max_stale_ms, 200);
    assert_eq!(glasses_class(&halo), Some(GlassesClass::MonoHud));
    assert!(has_symbol(&halo.display, "mono"));
    assert!(has_symbol(&halo.display, "hud"));
    assert!(has_symbol(&halo.output, "audio"));
    assert!(has_symbol(&halo.output, "haptic"));
    assert!(has_symbol(&halo.links, "bluetooth"));
    assert!(has_symbol(&halo.links, "web-bluetooth"));
    assert!(has_symbol(&halo.links, "phone-relay"));

    let halo_camera = SurfaceCaps::from_preset("glasses-hud-camera", "halo.local.camera")
        .expect("halo camera")
        .device_profile();
    assert_eq!(halo_camera.tier, DeviceTier::Actuator);
    assert_eq!(glasses_class(&halo_camera), Some(GlassesClass::MonoHud));
    assert!(has_symbol(&halo_camera.input, "camera"));
    assert!(has_symbol(&halo_camera.streams, "camera"));

    let three_dof = SurfaceCaps::from_preset("glasses-3dof", "viture.local.3dof")
        .expect("3dof glasses")
        .device_profile();
    assert_eq!(three_dof.tier, DeviceTier::Sensor);
    assert_eq!(three_dof.rate.content_hz, 60);
    assert_eq!(three_dof.rate.adapt_hz, 120);
    assert_eq!(glasses_class(&three_dof), Some(GlassesClass::DisplayOnly));
    assert!(has_symbol(&three_dof.display, "3dof"));
    assert!(has_symbol(&three_dof.streams, "motion"));

    let stereo = SurfaceCaps::from_preset("glasses-stereo", "viture.local.stereo")
        .expect("stereo display")
        .device_profile();
    assert_eq!(stereo.tier, DeviceTier::Display);
    assert_eq!(stereo.rate, RateClass::safe_default());
    assert_eq!(glasses_class(&stereo), Some(GlassesClass::DisplayOnly));
    assert!(has_symbol(&stereo.display, "stereo"));

    let viture = SurfaceCaps::from_preset("glasses-luma-ultra", "viture.local.ultra")
        .expect("luma ultra")
        .device_profile();
    assert_eq!(viture.tier, DeviceTier::Rich);
    assert_eq!(viture.rate.content_hz, 60);
    assert_eq!(viture.rate.adapt_hz, 120);
    assert_eq!(viture.rate.max_stale_ms, 25);
    assert_eq!(glasses_class(&viture), Some(GlassesClass::Stereo6Dof));
    assert!(has_symbol(&viture.display, "stereo"));
    assert!(has_symbol(&viture.display, "6dof"));
    assert!(has_symbol(&viture.streams, "pose"));
    assert!(has_symbol(&viture.streams, "hand"));
    assert!(has_symbol(&viture.input, "gaze"));
    assert!(has_symbol(&viture.links, "usb"));

    let degradation =
        DegradationResolver::resolve(&viture, &ObservedRoute::from_profile(&display_only));
    assert_eq!(degradation.tier, DeviceTier::Display);
    assert!(
        degradation
            .reasons
            .iter()
            .any(|reason| reason == "missing stream: pose")
    );

    let phone = SurfaceCaps::from_preset("phone", "phone.local")
        .expect("phone")
        .device_profile();
    assert_eq!(glasses_class(&phone), None);
}

#[test]
fn missing_sensor_field_degrades_with_reason() {
    let requested = tier_preset(DeviceTier::Rich);
    let mut observed = ObservedRoute::from_profile(&requested);
    observed
        .streams
        .retain(|symbol| symbol.name.as_ref() != "pose");

    let degradation = DegradationResolver::resolve(&requested, &observed);

    assert_eq!(degradation.tier, DeviceTier::Actuator);
    assert!(
        degradation
            .reasons
            .iter()
            .any(|reason| reason == "missing stream: pose")
    );
}

#[test]
fn missing_rate_map_defaults_to_safe_envelope() {
    let mut entries = match SurfaceCaps::from_preset("glasses-hud", "halo.local.2")
        .expect("halo caps")
        .to_expr()
    {
        Expr::Map(entries) => entries,
        _ => unreachable!(),
    };
    entries
        .retain(|(key, _)| !matches!(key, Expr::Symbol(symbol) if symbol.name.as_ref() == "rate"));

    let caps = SurfaceCaps::from_expr(&Expr::Map(entries)).expect("older caps parse");

    assert_eq!(caps.device_rate(), RateClass::safe_default());
    assert_eq!(
        access::field(&caps.rate, "content-hz"),
        access::field(&RateClass::safe_default().to_expr(), "content-hz")
    );
}

#[test]
fn trex3pro_worn_caps_fixture_separates_claims_from_verified() {
    assert_eq!(worn_caps_fixture_names(), [T_REX_3_PRO_48_CAPS_FIXTURE]);
    assert_eq!(
        worn_caps_fixture(T_REX_3_PRO_48_CAPS_FIXTURE),
        Some(trex3pro_48_worn_caps_fixture())
    );
    assert!(worn_caps_fixture("unknown").is_none());

    let fixture = trex3pro_48_worn_caps_fixture();
    assert_eq!(
        access::field(&fixture, "kind"),
        Some(&Expr::Symbol(Symbol::qualified(
            WORN_CAPS_NAMESPACE,
            WORN_CAPS_KIND
        )))
    );
    assert_eq!(
        access::field(&fixture, "device"),
        Some(&Expr::Symbol(Symbol::new(T_REX_3_PRO_48_CAPS_FIXTURE)))
    );

    let claims = access::field(&fixture, "claims").expect("claims map");
    assert_eq!(
        access::field(claims, "size-mm"),
        Some(&sim_value::build::uint(48))
    );
    assert_eq!(
        access::field(claims, "keys"),
        Some(&sim_value::build::uint(4))
    );
    assert_eq!(access::field(claims, "ble-hr"), Some(&Expr::Bool(true)));
    assert_eq!(
        access::field(claims, "notification-out"),
        Some(&Expr::Bool(true))
    );

    let verified = access::field(&fixture, "verified").expect("verified map");
    let Expr::Map(entries) = verified else {
        panic!("verified must be a map");
    };
    assert!(!entries.is_empty());
    for (_, value) in entries {
        assert_eq!(value, &Expr::Bool(false));
    }

    assert_eq!(access::field(&fixture, "firmware"), Some(&Expr::Nil));
    assert_eq!(
        RateClass::from_expr(access::field(&fixture, "rate").expect("rate map")).unwrap(),
        RateClass::watch()
    );
}

fn has_symbol(symbols: &[Symbol], name: &str) -> bool {
    symbols
        .iter()
        .any(|symbol| symbol.namespace.is_none() && symbol.name.as_ref() == name)
}

fn assert_symbol_subset(subset: &[Symbol], superset: &[Symbol]) {
    for symbol in subset {
        assert!(
            superset.iter().any(|candidate| candidate == symbol),
            "{} missing from superset",
            symbol.as_qualified_str()
        );
    }
}
