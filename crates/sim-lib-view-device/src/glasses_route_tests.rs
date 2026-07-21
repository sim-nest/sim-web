use crate::{DeviceTier, GlassesRoute, ObservedGlassesDevices, derive_tier, resolve_glasses_route};

#[test]
fn glasses_routes_degrade_honestly() {
    let no_accessory = resolve_glasses_route(
        GlassesRoute::DirectLinux,
        &ObservedGlassesDevices::default(),
    );
    assert_eq!(no_accessory.tier(), DeviceTier::Display);
    assert!(has_reason(&no_accessory, "Luma Ultra not observed"));

    let neckband = resolve_glasses_route(
        GlassesRoute::NeckbandLocal,
        &ObservedGlassesDevices {
            luma_ultra: true,
            neckband: true,
            ..ObservedGlassesDevices::default()
        },
    );
    assert_eq!(neckband.tier(), DeviceTier::Rich);
    assert!(neckband.degradation.reasons.is_empty());

    let dock = resolve_glasses_route(
        GlassesRoute::MobileDockDisplay,
        &ObservedGlassesDevices {
            luma_ultra: true,
            mobile_dock: true,
            ..ObservedGlassesDevices::default()
        },
    );
    assert_eq!(dock.tier(), DeviceTier::Display);
    assert!(has_reason(&dock, "Mobile Dock is display-only"));

    let halo_ble = resolve_glasses_route(
        GlassesRoute::HaloBleDirect,
        &ObservedGlassesDevices {
            halo: true,
            ..ObservedGlassesDevices::default()
        },
    );
    assert_eq!(halo_ble.tier(), DeviceTier::Actuator);
    assert!(has_reason(&halo_ble, "camera lane not observed"));

    let halo_relay = resolve_glasses_route(
        GlassesRoute::HaloPhoneRelay,
        &ObservedGlassesDevices {
            halo: true,
            halo_camera: true,
            ..ObservedGlassesDevices::default()
        },
    );
    assert_eq!(halo_relay.tier(), DeviceTier::Actuator);
    assert!(halo_relay.degradation.reasons.is_empty());

    let controller = resolve_glasses_route(
        GlassesRoute::ControllerHid,
        &ObservedGlassesDevices {
            controller_hid: true,
            ..ObservedGlassesDevices::default()
        },
    );
    assert_eq!(controller.tier(), DeviceTier::Display);
    assert!(
        controller
            .profile
            .input
            .iter()
            .any(|symbol| symbol.name.as_ref() == "controller")
    );
    assert!(has_reason(&controller, "ordinary HID/Intent"));
}

#[test]
fn every_glasses_route_uses_derived_tier_and_stable_symbol() {
    let routes = [
        GlassesRoute::DirectLinux,
        GlassesRoute::AndroidUsb,
        GlassesRoute::NeckbandLocal,
        GlassesRoute::NeckbandRelay,
        GlassesRoute::MobileDockDisplay,
        GlassesRoute::HaloBleDirect,
        GlassesRoute::HaloWebBluetooth,
        GlassesRoute::HaloPhoneRelay,
        GlassesRoute::ControllerHid,
    ];
    let seen = ObservedGlassesDevices {
        luma_ultra: true,
        sensor_usb: true,
        neckband: true,
        mobile_dock: true,
        halo: true,
        halo_camera: true,
        controller_hid: true,
    };

    for route in routes {
        let resolved = resolve_glasses_route(route, &seen);
        assert_eq!(resolved.profile.tier, derive_tier(&resolved.profile));
        assert_eq!(resolved.degradation.tier, resolved.profile.tier);
        assert_eq!(route.symbol().name.as_ref(), route.token());
        assert_eq!(route.symbol().namespace.as_deref(), Some("device/route"));
    }
}

fn has_reason(resolved: &crate::ResolvedGlassesProfile, needle: &str) -> bool {
    resolved
        .degradation
        .reasons
        .iter()
        .any(|reason| reason.contains(needle))
}
