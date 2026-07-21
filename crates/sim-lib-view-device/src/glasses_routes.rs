//! Glasses route, accessory, and capability resolution.

use sim_kernel::Symbol;
use sim_lib_view::SurfaceCaps;

use crate::{
    Degradation, DegradationResolver, DeviceProfile, DeviceProfileParts, DeviceTier, ObservedRoute,
    derive_tier,
};

/// Routes supported by the glasses profile resolver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassesRoute {
    /// Viture attached directly to a Linux host.
    DirectLinux,
    /// Viture sensors forwarded over Android USB.
    AndroidUsb,
    /// Viture attached to a locally executing neckband.
    NeckbandLocal,
    /// Viture attached through a neckband relay.
    NeckbandRelay,
    /// Mobile Dock display route, with sensors when separately visible.
    MobileDockDisplay,
    /// Halo attached over direct BLE.
    HaloBleDirect,
    /// Halo attached through Web Bluetooth.
    HaloWebBluetooth,
    /// Halo attached through a phone relay.
    HaloPhoneRelay,
    /// Ordinary HID controller input shared by either glasses family.
    ControllerHid,
}

impl GlassesRoute {
    /// Returns the stable command-line and route-symbol token.
    pub const fn token(self) -> &'static str {
        match self {
            Self::DirectLinux => "direct-linux",
            Self::AndroidUsb => "android-usb",
            Self::NeckbandLocal => "neckband-local",
            Self::NeckbandRelay => "neckband-relay",
            Self::MobileDockDisplay => "mobile-dock-display",
            Self::HaloBleDirect => "ble-direct",
            Self::HaloWebBluetooth => "web-bluetooth",
            Self::HaloPhoneRelay => "phone-relay",
            Self::ControllerHid => "controller-hid",
        }
    }

    /// Returns the stable route symbol.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("device/route", self.token())
    }
}

/// Devices and accessories visible while resolving a glasses route.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObservedGlassesDevices {
    /// A Viture Luma Ultra display is visible.
    pub luma_ultra: bool,
    /// The Viture sensor USB interface is visible.
    pub sensor_usb: bool,
    /// A Viture neckband is visible.
    pub neckband: bool,
    /// A Viture Mobile Dock is visible.
    pub mobile_dock: bool,
    /// A Halo display is visible.
    pub halo: bool,
    /// The Halo camera lane is available.
    pub halo_camera: bool,
    /// An ordinary HID controller is visible.
    pub controller_hid: bool,
}

/// Highest supported profile and degradation evidence for one route.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedGlassesProfile {
    /// Route that was resolved.
    pub route: GlassesRoute,
    /// Profile supported by the observed route and accessories.
    pub profile: DeviceProfile,
    /// Missing requested capabilities and route-specific fallback reasons.
    pub degradation: Degradation,
}

impl ResolvedGlassesProfile {
    /// Returns the authoritative tier derived from the resolved profile.
    pub fn tier(&self) -> DeviceTier {
        derive_tier(&self.profile)
    }
}

/// Resolves a glasses route to its highest supported profile and honest reasons.
pub fn resolve_glasses_route(
    route: GlassesRoute,
    seen: &ObservedGlassesDevices,
) -> ResolvedGlassesProfile {
    match route {
        GlassesRoute::DirectLinux | GlassesRoute::AndroidUsb => resolve_viture_usb(route, seen),
        GlassesRoute::NeckbandLocal | GlassesRoute::NeckbandRelay => {
            resolve_viture_neckband(route, seen)
        }
        GlassesRoute::MobileDockDisplay => resolve_mobile_dock(route, seen),
        GlassesRoute::HaloBleDirect
        | GlassesRoute::HaloWebBluetooth
        | GlassesRoute::HaloPhoneRelay => resolve_halo(route, seen),
        GlassesRoute::ControllerHid => resolve_controller(route, seen),
    }
}

fn resolve_viture_usb(
    route: GlassesRoute,
    seen: &ObservedGlassesDevices,
) -> ResolvedGlassesProfile {
    let requested = route_profile("glasses-luma-ultra", route);
    if seen.luma_ultra && seen.sensor_usb {
        return resolved(route, requested.clone(), requested, None);
    }
    let profile = route_profile(
        if seen.luma_ultra {
            "glasses-stereo"
        } else {
            "glasses"
        },
        route,
    );
    let reason = if seen.luma_ultra {
        "sensor USB not visible; route is display-only"
    } else {
        "Luma Ultra not observed; route uses a generic display profile"
    };
    resolved(route, requested, profile, Some(reason))
}

fn resolve_viture_neckband(
    route: GlassesRoute,
    seen: &ObservedGlassesDevices,
) -> ResolvedGlassesProfile {
    let requested = route_profile("glasses-luma-ultra", route);
    if seen.luma_ultra && seen.neckband {
        return resolved(route, requested.clone(), requested, None);
    }
    let profile = route_profile(
        if seen.luma_ultra {
            "glasses-stereo"
        } else {
            "glasses"
        },
        route,
    );
    resolved(
        route,
        requested,
        profile,
        Some("neckband sensor path not observed; route is display-only"),
    )
}

fn resolve_mobile_dock(
    route: GlassesRoute,
    seen: &ObservedGlassesDevices,
) -> ResolvedGlassesProfile {
    let requested = route_profile("glasses-luma-ultra", route);
    if seen.luma_ultra && seen.mobile_dock && seen.sensor_usb {
        return resolved(route, requested.clone(), requested, None);
    }
    let profile = route_profile(
        if seen.luma_ultra && seen.mobile_dock {
            "glasses-stereo"
        } else {
            "glasses"
        },
        route,
    );
    let reason = if seen.mobile_dock && !seen.sensor_usb {
        "sensor USB not visible; Mobile Dock is display-only"
    } else {
        "Mobile Dock display path not observed; route uses a generic display profile"
    };
    resolved(route, requested, profile, Some(reason))
}

fn resolve_halo(route: GlassesRoute, seen: &ObservedGlassesDevices) -> ResolvedGlassesProfile {
    let requested = route_profile("glasses-hud-camera", route);
    if !seen.halo {
        return resolved(
            route,
            requested,
            route_profile("glasses", route),
            Some("Halo not observed; route uses a generic display profile"),
        );
    }
    let profile = route_profile(
        if seen.halo_camera {
            "glasses-hud-camera"
        } else {
            "glasses-hud"
        },
        route,
    );
    let reason = (!seen.halo_camera).then_some("Halo camera lane not observed");
    resolved(route, requested, profile, reason)
}

fn resolve_controller(
    route: GlassesRoute,
    seen: &ObservedGlassesDevices,
) -> ResolvedGlassesProfile {
    let requested = controller_profile(route, true);
    let profile = controller_profile(route, seen.controller_hid);
    let reason = if seen.controller_hid {
        Some("controller remains ordinary HID/Intent and does not raise the glasses tier")
    } else {
        Some("controller HID not observed")
    };
    resolved(route, requested, profile, reason)
}

fn resolved(
    route: GlassesRoute,
    requested: DeviceProfile,
    profile: DeviceProfile,
    reason: Option<&str>,
) -> ResolvedGlassesProfile {
    let mut degradation =
        DegradationResolver::resolve(&requested, &ObservedRoute::from_profile(&profile));
    if let Some(reason) = reason {
        degradation.reasons.push(reason.to_owned());
    }
    degradation.tier = derive_tier(&profile);
    ResolvedGlassesProfile {
        route,
        profile,
        degradation,
    }
}

fn route_profile(preset: &str, route: GlassesRoute) -> DeviceProfile {
    let caps =
        SurfaceCaps::from_preset(preset, route.token()).expect("built-in glasses profile preset");
    let profile = DeviceProfile::from_surface_caps(&caps);
    with_route_fields(profile, route, Vec::new())
}

fn controller_profile(route: GlassesRoute, controller_hid: bool) -> DeviceProfile {
    let caps = SurfaceCaps::from_preset("glasses", route.token())
        .expect("built-in glasses profile preset");
    let profile = DeviceProfile::from_surface_caps(&caps);
    let input = controller_hid
        .then(|| Symbol::new("controller"))
        .into_iter()
        .collect();
    with_route_fields(profile, route, input)
}

fn with_route_fields(
    profile: DeviceProfile,
    route: GlassesRoute,
    mut extra_input: Vec<Symbol>,
) -> DeviceProfile {
    extra_input.extend(profile.input);
    DeviceProfile::new(DeviceProfileParts {
        kind: profile.kind,
        display: profile.display,
        input: extra_input,
        output: profile.output,
        links: vec![Symbol::new(route.token())],
        streams: profile.streams,
        rate: profile.rate,
        policy: profile.policy,
    })
}
