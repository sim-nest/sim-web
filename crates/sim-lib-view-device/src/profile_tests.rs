use sim_kernel::Expr;
use sim_lib_view::SurfaceCaps;
use sim_value::access;

use crate::{
    DegradationResolver, DeviceProfile, DeviceSurfaceCapsExt, DeviceTier, ObservedRoute, RateClass,
    derive_tier, tier_preset,
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
    let mut entries = match SurfaceCaps::from_preset("watch", "watch.local.2")
        .expect("watch caps")
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
