//! Typed device profiles over open surface capability maps.

use sim_kernel::{Expr, Symbol};
use sim_lib_view::SurfaceCaps;
use sim_value::{access, build};

use crate::ladder::DeviceTier;
use crate::rate::{RateClass, RateError};

/// The namespace for serialized device profile records.
pub const DEVICE_PROFILE_NAMESPACE: &str = "device";

/// The `kind` tag of a serialized [`DeviceProfile`] map.
pub const DEVICE_PROFILE_KIND: &str = "profile";

/// A typed device envelope derived from [`SurfaceCaps`].
///
/// This is capability data, not a concrete device enum. Instances advertise
/// maps; this profile reads them into stable lanes so routing and degradation
/// decisions share one interpretation.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceProfile {
    /// Device family token, for example `watch`, `glasses`, or `desktop`.
    pub kind: Symbol,
    /// Tier computed by [`derive_tier`].
    pub tier: DeviceTier,
    /// Display capability tokens such as `flat`, `stereo`, `round`, or `hud`.
    pub display: Vec<Symbol>,
    /// Input capability tokens such as `touch`, `tap`, `voice`, or `crown`.
    pub input: Vec<Symbol>,
    /// Output capability tokens such as `screen`, `haptic`, `speaker`, or `hud`.
    pub output: Vec<Symbol>,
    /// Link tokens such as `usb`, `phone-relay`, or `lan-ws`.
    pub links: Vec<Symbol>,
    /// Stream tokens such as `pose`, `heart-rate`, `motion`, or `battery`.
    pub streams: Vec<Symbol>,
    /// Declared timing envelope.
    pub rate: RateClass,
    /// Consent, retention, and redaction hints.
    pub policy: Expr,
}

/// Field bundle for constructing a [`DeviceProfile`].
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceProfileParts {
    /// Device family token, for example `watch`, `glasses`, or `desktop`.
    pub kind: Symbol,
    /// Display capability tokens.
    pub display: Vec<Symbol>,
    /// Input capability tokens.
    pub input: Vec<Symbol>,
    /// Output capability tokens.
    pub output: Vec<Symbol>,
    /// Link tokens.
    pub links: Vec<Symbol>,
    /// Stream tokens.
    pub streams: Vec<Symbol>,
    /// Declared timing envelope.
    pub rate: RateClass,
    /// Consent, retention, and redaction hints.
    pub policy: Expr,
}

impl DeviceProfile {
    /// Builds a profile and computes the tier from the advertised fields.
    pub fn new(parts: DeviceProfileParts) -> Self {
        let mut profile = Self {
            kind: parts.kind,
            tier: DeviceTier::Display,
            display: dedup_symbols(parts.display),
            input: dedup_symbols(parts.input),
            output: dedup_symbols(parts.output),
            links: dedup_symbols(parts.links),
            streams: dedup_symbols(parts.streams),
            rate: parts.rate,
            policy: parts.policy,
        };
        profile.tier = derive_tier(&profile);
        profile
    }

    /// Derives a device profile from open [`SurfaceCaps`] metadata.
    pub fn from_surface_caps(caps: &SurfaceCaps) -> Self {
        let display = display_symbols(&caps.display);
        let input = flag_symbols(&caps.input);
        let output = output_symbols(&display, &input);
        let links = link_symbols(&caps.transport);
        let streams = stream_symbols(&caps.input);
        let rate = RateClass::from_expr(&caps.rate).unwrap_or_else(|_| RateClass::safe_default());
        Self::new(DeviceProfileParts {
            kind: Symbol::new(caps.preset_name().to_owned()),
            display,
            input,
            output,
            links,
            streams,
            rate,
            policy: caps.privacy.clone(),
        })
    }

    /// Encodes this profile as a `device/profile` tagged map.
    pub fn to_expr(&self) -> Expr {
        build::map(vec![
            (
                "kind",
                Expr::Symbol(Symbol::qualified(
                    DEVICE_PROFILE_NAMESPACE,
                    DEVICE_PROFILE_KIND,
                )),
            ),
            ("device-kind", Expr::Symbol(self.kind.clone())),
            ("tier", Expr::Symbol(self.tier.to_symbol())),
            ("display", symbol_list(&self.display)),
            ("input", symbol_list(&self.input)),
            ("output", symbol_list(&self.output)),
            ("links", symbol_list(&self.links)),
            ("streams", symbol_list(&self.streams)),
            ("rate", self.rate.to_expr()),
            ("policy", self.policy.clone()),
        ])
    }

    /// Parses a `device/profile` tagged map.
    pub fn from_expr(expr: &Expr) -> Result<Self, DeviceProfileError> {
        let Expr::Map(entries) = expr else {
            return Err(DeviceProfileError::NotProfile);
        };
        match access::entry_field(entries, "kind") {
            Some(Expr::Symbol(kind))
                if kind.namespace.as_deref() == Some(DEVICE_PROFILE_NAMESPACE)
                    && kind.name.as_ref() == DEVICE_PROFILE_KIND => {}
            _ => return Err(DeviceProfileError::NotProfile),
        }
        let kind = match access::entry_field(entries, "device-kind") {
            Some(Expr::Symbol(symbol)) => symbol.clone(),
            Some(_) => return Err(DeviceProfileError::BadField("device-kind")),
            None => return Err(DeviceProfileError::MissingField("device-kind")),
        };
        let tier = match access::entry_field(entries, "tier") {
            Some(Expr::Symbol(symbol)) => {
                DeviceTier::from_symbol(symbol).ok_or(DeviceProfileError::BadField("tier"))?
            }
            Some(_) => return Err(DeviceProfileError::BadField("tier")),
            None => return Err(DeviceProfileError::MissingField("tier")),
        };
        let display = symbol_vec(entries, "display")?;
        let input = symbol_vec(entries, "input")?;
        let output = symbol_vec(entries, "output")?;
        let links = symbol_vec(entries, "links")?;
        let streams = symbol_vec(entries, "streams")?;
        let rate = match access::entry_field(entries, "rate") {
            Some(value) => RateClass::from_expr(value).map_err(DeviceProfileError::Rate)?,
            None => return Err(DeviceProfileError::MissingField("rate")),
        };
        let policy = match access::entry_field(entries, "policy") {
            Some(value) => value.clone(),
            None => return Err(DeviceProfileError::MissingField("policy")),
        };
        let profile = Self {
            kind,
            tier,
            display: dedup_symbols(display),
            input: dedup_symbols(input),
            output: dedup_symbols(output),
            links: dedup_symbols(links),
            streams: dedup_symbols(streams),
            rate,
            policy,
        };
        let derived = derive_tier(&profile);
        if tier != derived {
            return Err(DeviceProfileError::TierMismatch {
                declared: tier,
                derived,
            });
        }
        Ok(profile)
    }
}

/// Extension methods that view [`SurfaceCaps`] as a device profile.
pub trait DeviceSurfaceCapsExt {
    /// Builds the typed profile for these surface capabilities.
    fn device_profile(&self) -> DeviceProfile;

    /// Reads the timing envelope, using the safe default when missing or
    /// malformed.
    fn device_rate(&self) -> RateClass;
}

impl DeviceSurfaceCapsExt for SurfaceCaps {
    fn device_profile(&self) -> DeviceProfile {
        DeviceProfile::from_surface_caps(self)
    }

    fn device_rate(&self) -> RateClass {
        RateClass::from_expr(&self.rate).unwrap_or_else(|_| RateClass::safe_default())
    }
}

/// The single authoritative tier derivation function.
pub fn derive_tier(profile: &DeviceProfile) -> DeviceTier {
    if has_symbol(&profile.display, "stereo") && has_symbol(&profile.streams, "pose") {
        DeviceTier::Rich
    } else if has_any(
        &profile.output,
        &["haptic", "face", "hud", "speaker", "tone"],
    ) {
        DeviceTier::Actuator
    } else if !profile.streams.is_empty() {
        DeviceTier::Sensor
    } else {
        DeviceTier::Display
    }
}

/// Returns a compact preset profile for the requested tier.
pub fn tier_preset(tier: DeviceTier) -> DeviceProfile {
    match tier {
        DeviceTier::Display => DeviceProfile::new(DeviceProfileParts {
            kind: Symbol::new("display"),
            display: symbols(&["flat"]),
            input: Vec::new(),
            output: symbols(&["screen"]),
            links: symbols(&["usb"]),
            streams: Vec::new(),
            rate: RateClass::safe_default(),
            policy: build::map(Vec::new()),
        }),
        DeviceTier::Sensor => DeviceProfile::new(DeviceProfileParts {
            kind: Symbol::new("sensor"),
            display: symbols(&["flat"]),
            input: Vec::new(),
            output: Vec::new(),
            links: symbols(&["bluetooth"]),
            streams: symbols(&["motion"]),
            rate: RateClass::watch(),
            policy: build::map(Vec::new()),
        }),
        DeviceTier::Actuator => DeviceProfile::new(DeviceProfileParts {
            kind: Symbol::new("actuator"),
            display: symbols(&["round"]),
            input: symbols(&["tap"]),
            output: symbols(&["haptic"]),
            links: symbols(&["phone-relay"]),
            streams: symbols(&["battery"]),
            rate: RateClass::watch(),
            policy: build::map(Vec::new()),
        }),
        DeviceTier::Rich => DeviceProfile::new(DeviceProfileParts {
            kind: Symbol::new("rich"),
            display: symbols(&["stereo", "hud"]),
            input: symbols(&["voice"]),
            output: symbols(&["hud", "speaker"]),
            links: symbols(&["lan-ws"]),
            streams: symbols(&["pose", "motion"]),
            rate: RateClass::stereo(),
            policy: build::map(Vec::new()),
        }),
    }
}

/// A small expression used by the embedded recipe.
pub fn device_profile_demo() -> Expr {
    tier_preset(DeviceTier::Rich).to_expr()
}

/// A reason a device profile could not be parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceProfileError {
    /// The value was not a `device/profile` tagged map.
    NotProfile,
    /// A required field was missing.
    MissingField(&'static str),
    /// A field carried the wrong value shape.
    BadField(&'static str),
    /// The rate map was malformed.
    Rate(RateError),
    /// The declared tier disagreed with [`derive_tier`].
    TierMismatch {
        /// The tier declared in the profile map.
        declared: DeviceTier,
        /// The tier derived from the capability fields.
        derived: DeviceTier,
    },
}

impl core::fmt::Display for DeviceProfileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DeviceProfileError::NotProfile => write!(f, "value is not a device/profile map"),
            DeviceProfileError::MissingField(name) => {
                write!(f, "device profile missing field: {name}")
            }
            DeviceProfileError::BadField(name) => {
                write!(f, "device profile field has wrong shape: {name}")
            }
            DeviceProfileError::Rate(err) => write!(f, "device profile rate: {err}"),
            DeviceProfileError::TierMismatch { declared, derived } => write!(
                f,
                "device profile tier mismatch: declared {}, derived {}",
                declared.token(),
                derived.token()
            ),
        }
    }
}

impl std::error::Error for DeviceProfileError {}

fn symbol_vec(
    entries: &[(Expr, Expr)],
    name: &'static str,
) -> Result<Vec<Symbol>, DeviceProfileError> {
    match access::entry_field(entries, name) {
        Some(Expr::List(items)) => items
            .iter()
            .map(|item| match item {
                Expr::Symbol(symbol) => Ok(symbol.clone()),
                _ => Err(DeviceProfileError::BadField(name)),
            })
            .collect(),
        Some(_) => Err(DeviceProfileError::BadField(name)),
        None => Err(DeviceProfileError::MissingField(name)),
    }
}

fn symbol_list(symbols: &[Symbol]) -> Expr {
    build::list(symbols.iter().cloned().map(Expr::Symbol).collect())
}

fn display_symbols(display: &Expr) -> Vec<Symbol> {
    let mut out = Vec::new();
    if matches!(access::field(display, "stereo"), Some(Expr::Bool(true))) {
        push_symbol(&mut out, "stereo");
    }
    if access::field(display, "lines").is_some() {
        push_symbol(&mut out, "hud");
    }
    push_symbol_field(&mut out, display, "shape");
    push_symbol_field(&mut out, display, "density");
    if out.is_empty() && matches!(display, Expr::Map(entries) if !entries.is_empty()) {
        push_symbol(&mut out, "flat");
    }
    out
}

fn flag_symbols(map: &Expr) -> Vec<Symbol> {
    let Expr::Map(entries) = map else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, value) in entries {
        if !matches!(value, Expr::Bool(true)) {
            continue;
        }
        if let Expr::Symbol(symbol) = key
            && symbol.namespace.is_none()
        {
            push_existing(&mut out, symbol.clone());
        }
    }
    out
}

fn output_symbols(display: &[Symbol], input: &[Symbol]) -> Vec<Symbol> {
    let mut out = Vec::new();
    if !has_symbol(display, "none") && !display.is_empty() {
        push_symbol(&mut out, "screen");
    }
    if has_symbol(display, "hud") || has_symbol(display, "stereo") {
        push_symbol(&mut out, "hud");
    }
    if has_symbol(input, "haptic-ack") {
        push_symbol(&mut out, "haptic");
    }
    out
}

fn link_symbols(transport: &Expr) -> Vec<Symbol> {
    let mut out = Vec::new();
    match access::field(transport, "kind") {
        Some(Expr::Symbol(kind)) if kind.name.as_ref() == "relay" => {
            push_symbol(&mut out, "phone-relay");
        }
        Some(Expr::Symbol(kind)) if kind.name.as_ref() == "websocket" => {
            push_symbol(&mut out, "lan-ws");
        }
        Some(Expr::Symbol(kind)) if matches!(kind.name.as_ref(), "tty" | "local") => {
            push_symbol(&mut out, "usb");
        }
        Some(Expr::Symbol(kind)) => push_existing(&mut out, kind.clone()),
        _ => {}
    }
    out
}

fn stream_symbols(input: &Expr) -> Vec<Symbol> {
    let mut out = Vec::new();
    if matches!(access::field(input, "camera"), Some(Expr::Bool(true))) {
        push_symbol(&mut out, "motion");
    }
    out
}

fn push_symbol_field(out: &mut Vec<Symbol>, map: &Expr, name: &str) {
    if let Some(Expr::Symbol(symbol)) = access::field(map, name) {
        push_existing(out, symbol.clone());
    }
}

fn symbols(names: &[&str]) -> Vec<Symbol> {
    names.iter().map(|name| Symbol::new(*name)).collect()
}

pub(crate) fn has_symbol(symbols: &[Symbol], name: &str) -> bool {
    symbols
        .iter()
        .any(|symbol| symbol.namespace.is_none() && symbol.name.as_ref() == name)
}

pub(crate) fn has_any(symbols: &[Symbol], names: &[&str]) -> bool {
    names.iter().any(|name| has_symbol(symbols, name))
}

pub(crate) fn push_symbol(out: &mut Vec<Symbol>, name: &str) {
    push_existing(out, Symbol::new(name.to_owned()));
}

pub(crate) fn push_existing(out: &mut Vec<Symbol>, symbol: Symbol) {
    if !out.iter().any(|existing| existing == &symbol) {
        out.push(symbol);
    }
}

fn dedup_symbols(symbols: Vec<Symbol>) -> Vec<Symbol> {
    let mut out = Vec::new();
    for symbol in symbols {
        push_existing(&mut out, symbol);
    }
    out
}
