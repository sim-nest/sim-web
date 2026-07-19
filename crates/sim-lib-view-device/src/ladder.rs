//! Ordered device tiers derived from open surface metadata.

use sim_kernel::Symbol;

/// The capability ladder for small and wearable surfaces.
///
/// The order is meaningful: a higher tier can satisfy requirements for any
/// lower tier. The variants are not concrete device kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceTier {
    /// A display-only surface can show a reduced glance or pane.
    Display,
    /// A sensor surface contributes live stream fields but does not act back.
    Sensor,
    /// An actuator surface can emit haptics, HUD output, face output, or sound.
    Actuator,
    /// A rich surface combines high-rate display with pose or equivalent stream
    /// data.
    Rich,
}

impl DeviceTier {
    /// Every tier in ascending capability order.
    pub const ALL: [DeviceTier; 4] = [
        DeviceTier::Display,
        DeviceTier::Sensor,
        DeviceTier::Actuator,
        DeviceTier::Rich,
    ];

    /// The stable token used in profile expressions.
    pub fn token(self) -> &'static str {
        match self {
            DeviceTier::Display => "display",
            DeviceTier::Sensor => "sensor",
            DeviceTier::Actuator => "actuator",
            DeviceTier::Rich => "rich",
        }
    }

    /// Encodes the tier as an unqualified symbol.
    pub fn to_symbol(self) -> Symbol {
        Symbol::new(self.token())
    }

    /// Parses an unqualified tier symbol.
    pub fn from_symbol(symbol: &Symbol) -> Option<Self> {
        match symbol.name.as_ref() {
            "display" if symbol.namespace.is_none() => Some(DeviceTier::Display),
            "sensor" if symbol.namespace.is_none() => Some(DeviceTier::Sensor),
            "actuator" if symbol.namespace.is_none() => Some(DeviceTier::Actuator),
            "rich" if symbol.namespace.is_none() => Some(DeviceTier::Rich),
            _ => None,
        }
    }

    /// Returns true when this tier can satisfy a requirement for `required`.
    pub fn supports(self, required: DeviceTier) -> bool {
        self >= required
    }
}
