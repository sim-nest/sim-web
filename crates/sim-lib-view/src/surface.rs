//! Surface capability metadata -- the library-level "surface" output position.
//!
//! A view is a codec at output position `surface`. The kernel keeps its closed
//! [`sim_kernel::EncodePosition`] (`Eval`/`Quote`/`Data`/`Pattern`); the surface
//! position lives here as open metadata so a view/edit lens projects toward a
//! device described purely by capability data, never a
//! closed device enum. A surface advertises what it can show and accept; the
//! projection ranker (see [`crate::dispatch`]) reads those capabilities.
//!
//! [`SurfaceCaps`] round-trips through a `surface/caps` tagged [`Expr`] map, the
//! same shape SIM uses for [`Scene`](sim_lib_scene) and Intent values, so a
//! surface descriptor is itself an ordinary SIM value that can cross a session.
//!
//! # Example
//!
//! ```
//! use sim_lib_view::surface;
//!
//! let cli = surface::preset("cli").expect("cli is a known preset");
//! // Capabilities round-trip losslessly through their `surface/caps` Expr form.
//! let back = surface::SurfaceCaps::from_expr(&cli.to_expr()).unwrap();
//! assert_eq!(cli, back);
//! assert!(cli.input_flag("keyboard"));
//! ```

use sim_kernel::{Error, Expr, Symbol};
use sim_value::{access, build};

/// The metadata namespace for surface descriptors (`surface/...`).
pub const SURFACE_NAMESPACE: &str = "surface";

/// The `kind` tag of a serialized [`SurfaceCaps`] map.
pub const CAPS_KIND: &str = "caps";

/// The catalog of well-known surface presets, by unqualified name.
///
/// These are named capability bundles, NOT a runtime enum: a device that is not
/// in this list still works by advertising its own [`SurfaceCaps`]. The presets
/// exist so common surfaces have a one-line starting point.
pub const SURFACE_PRESETS: &[&str] = &[
    "cli",
    "tui",
    "webui",
    "watch",
    "watch-glance",
    "watch-glance-large",
    "watch-sport",
    "watch-sleep",
    "glasses",
    "glasses-hud",
    "glasses-hud-camera",
    "glasses-3dof",
    "glasses-stereo",
    "glasses-luma-ultra",
    "phone",
    "desktop",
];

/// A surface's advertised capabilities, as open metadata over [`Expr`].
///
/// The capability maps (`display`, `input`, `output`, `transport`, `privacy`,
/// `rate`, `streams`) are open: a surface may carry fields beyond the
/// well-known ones, and the ranker reads only the fields it understands.
/// `codecs` lists the surface codecs the client can decode (lisp/json/bin/...).
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceCaps {
    /// A stable client identifier, e.g. `"tty.local.1"`.
    pub client_id: String,
    /// The preset name this surface is based on (`surface/<preset>`).
    pub preset: Symbol,
    /// Display capabilities: cells/pixels, color, density, motion, budget.
    pub display: Expr,
    /// Input capabilities: keyboard/pointer/touch/voice/camera/tap/...
    pub input: Expr,
    /// Output capabilities: screen/haptic/face/tone/speaker/mic/...
    pub output: Expr,
    /// Transport capabilities: kind, round-trip, offline queue, ordering.
    pub transport: Expr,
    /// Privacy policy: redaction class, retention, private fields.
    pub privacy: Expr,
    /// Timing envelope: content cadence, adapter cadence, and staleness budget.
    pub rate: Expr,
    /// Stream capabilities: heart-rate/motion/location/battery/...
    pub streams: Expr,
    /// Surface codecs the client can decode, in preference order.
    pub codecs: Vec<Symbol>,
}

/// A reason a [`SurfaceCaps`] value could not be parsed from an [`Expr`].
///
/// Parsing fails closed: a malformed descriptor never yields partial caps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurfaceError {
    /// The value was not a `surface/caps`-tagged map.
    NotCaps,
    /// A required field was missing.
    MissingField(&'static str),
    /// A field carried the wrong value shape.
    BadField(&'static str),
    /// A shared `sim_value::access` slice reader rejected a field (e.g. a
    /// required field was missing). Carries the reader's rendered message so the
    /// surface decoder can lean on the shared readers without growing a variant
    /// per field. The reader's error is the kernel `Error` type that
    /// `sim_value::access` returns.
    Field(String),
}

impl core::fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SurfaceError::NotCaps => write!(f, "value is not a surface/caps map"),
            SurfaceError::MissingField(name) => write!(f, "surface caps missing field: {name}"),
            SurfaceError::BadField(name) => write!(f, "surface caps field has wrong shape: {name}"),
            SurfaceError::Field(message) => write!(f, "surface caps field: {message}"),
        }
    }
}

impl std::error::Error for SurfaceError {}

impl From<Error> for SurfaceError {
    /// Adopts a shared `sim_value::access` reader error (the kernel `Error` those
    /// readers return) as a surface parse failure, so `map_field` can defer
    /// required-field lookup to the substrate while the surface decoder keeps
    /// failing closed.
    fn from(err: Error) -> Self {
        SurfaceError::Field(err.to_string())
    }
}

impl SurfaceCaps {
    /// Builds caps from a preset name plus a concrete `client_id`.
    ///
    /// Returns `None` when `preset_name` is not in [`SURFACE_PRESETS`].
    pub fn from_preset(preset_name: &str, client_id: impl Into<String>) -> Option<Self> {
        let mut caps = preset(preset_name)?;
        caps.client_id = client_id.into();
        Some(caps)
    }

    /// Encodes these caps as a `surface/caps` tagged [`Expr`] map.
    pub fn to_expr(&self) -> Expr {
        build::map(vec![
            (
                "kind",
                Expr::Symbol(Symbol::qualified(SURFACE_NAMESPACE, CAPS_KIND)),
            ),
            ("client-id", build::text(self.client_id.clone())),
            ("preset", Expr::Symbol(self.preset.clone())),
            ("display", self.display.clone()),
            ("input", self.input.clone()),
            ("output", self.output.clone()),
            ("transport", self.transport.clone()),
            ("privacy", self.privacy.clone()),
            ("rate", self.rate.clone()),
            ("streams", self.streams.clone()),
            (
                "codecs",
                build::list(self.codecs.iter().cloned().map(Expr::Symbol).collect()),
            ),
        ])
    }

    /// Parses caps from a `surface/caps` tagged [`Expr`] map, failing closed.
    pub fn from_expr(expr: &Expr) -> Result<Self, SurfaceError> {
        let Expr::Map(entries) = expr else {
            return Err(SurfaceError::NotCaps);
        };
        match access::entry_field(entries, "kind") {
            Some(Expr::Symbol(kind))
                if kind.namespace.as_deref() == Some(SURFACE_NAMESPACE)
                    && &*kind.name == CAPS_KIND => {}
            _ => return Err(SurfaceError::NotCaps),
        }
        let client_id = match access::entry_field(entries, "client-id") {
            Some(Expr::String(text)) => text.clone(),
            Some(_) => return Err(SurfaceError::BadField("client-id")),
            None => return Err(SurfaceError::MissingField("client-id")),
        };
        let preset = match access::entry_field(entries, "preset") {
            Some(Expr::Symbol(symbol)) => symbol.clone(),
            Some(_) => return Err(SurfaceError::BadField("preset")),
            None => return Err(SurfaceError::MissingField("preset")),
        };
        let display = map_field(entries, "display")?;
        let input = map_field(entries, "input")?;
        let output = optional_map_field(entries, "output")?;
        let transport = map_field(entries, "transport")?;
        let privacy = map_field(entries, "privacy")?;
        let rate = match access::entry_field(entries, "rate") {
            Some(value @ Expr::Map(_)) => value.clone(),
            Some(_) => return Err(SurfaceError::BadField("rate")),
            None => rate_map(1, 1, 1000),
        };
        let streams = optional_map_field(entries, "streams")?;
        let codecs = match access::entry_field(entries, "codecs") {
            Some(Expr::List(items)) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    let Expr::Symbol(symbol) = item else {
                        return Err(SurfaceError::BadField("codecs"));
                    };
                    out.push(symbol.clone());
                }
                out
            }
            Some(_) => return Err(SurfaceError::BadField("codecs")),
            None => return Err(SurfaceError::MissingField("codecs")),
        };
        Ok(SurfaceCaps {
            client_id,
            preset,
            display,
            input,
            output,
            transport,
            privacy,
            rate,
            streams,
            codecs,
        })
    }

    /// Returns the unqualified preset name (`cli`, `watch`, ...).
    pub fn preset_name(&self) -> &str {
        &self.preset.name
    }

    /// Reads a boolean `input` capability flag, defaulting to `false`.
    pub fn input_flag(&self, name: &str) -> bool {
        matches!(access::field(&self.input, name), Some(Expr::Bool(true)))
    }

    /// Reads the `display` density symbol (`glance`/`compact`/`regular`/`dense`).
    pub fn display_density(&self) -> Option<Symbol> {
        match access::field(&self.display, "density") {
            Some(Expr::Symbol(symbol)) => Some(symbol.clone()),
            _ => None,
        }
    }

    /// Whether this surface can decode the named surface codec.
    pub fn accepts_codec(&self, codec: &str) -> bool {
        self.codecs.iter().any(|symbol| &*symbol.name == codec)
    }
}

/// Returns the baseline [`SurfaceCaps`] for a well-known preset name.
///
/// The `client_id` is set to the preset name and should be overridden with a
/// real id via [`SurfaceCaps::from_preset`]. Returns `None` for unknown presets.
pub fn preset(name: &str) -> Option<SurfaceCaps> {
    let (display, input, output, transport, privacy, rate, streams) = match name {
        "cli" => (
            display_map(&[("density", sym("dense")), ("color", sym("ansi"))]),
            input_map(&["keyboard"]),
            output_map(&["screen"]),
            transport_map("tty", 1, false),
            privacy_map("local", 60_000),
            rate_map(1, 1, 1000),
            streams_map(&[]),
        ),
        "tui" => (
            display_map(&[("density", sym("dense")), ("color", sym("ansi256"))]),
            input_map(&["keyboard", "pointer"]),
            output_map(&["screen"]),
            transport_map("tty", 1, false),
            privacy_map("local", 60_000),
            rate_map(1, 1, 1000),
            streams_map(&[]),
        ),
        "webui" => (
            display_map(&[("density", sym("regular")), ("color", sym("truecolor"))]),
            input_map(&["keyboard", "pointer", "touch", "wheel", "file-drop"]),
            output_map(&["screen"]),
            transport_map("websocket", 40, false),
            privacy_map("session", 600_000),
            rate_map(5, 30, 500),
            streams_map(&[]),
        ),
        "watch" => (
            watch_display_map("generic-round-watch", 480, 48),
            input_map(&[
                "button",
                "touch",
                "tap",
                "raise",
                "mic",
                "crown",
                "haptic-ack",
            ]),
            watch_output_map(),
            transport_map_with_links("phone-relay", 250, true, &["phone-relay", "ble"]),
            privacy_map("local", 60_000),
            watch_rate_map(),
            streams_map(&["heart-rate", "motion", "battery", "connection"]),
        ),
        "watch-glance" => (
            watch_display_map("amazfit-t-rex-3-pro-44", 466, 44),
            input_map(&["button", "touch", "tap", "raise", "mic", "haptic-ack"]),
            watch_output_map(),
            transport_map_with_links(
                "phone-relay",
                250,
                true,
                &["modeled", "phone-relay", "ble", "zepp-export"],
            ),
            privacy_map("local", 60_000),
            watch_rate_map(),
            streams_map(&[
                "heart-rate",
                "motion",
                "location",
                "environment",
                "battery",
                "connection",
            ]),
        ),
        "watch-glance-large" => (
            watch_display_map("amazfit-t-rex-3-pro-48", 480, 48),
            input_map(&["button", "touch", "tap", "raise", "mic", "haptic-ack"]),
            watch_output_map(),
            transport_map_with_links(
                "phone-relay",
                250,
                true,
                &["modeled", "phone-relay", "ble", "zepp-export"],
            ),
            privacy_map("local", 60_000),
            watch_rate_map(),
            streams_map(&[
                "heart-rate",
                "motion",
                "location",
                "environment",
                "battery",
                "connection",
            ]),
        ),
        "watch-sport" => (
            watch_display_map("amazfit-t-rex-3-pro-48", 480, 48),
            input_map(&["button", "touch", "tap", "raise", "mic", "haptic-ack"]),
            watch_output_map(),
            transport_map_with_links(
                "phone-relay",
                250,
                true,
                &["modeled", "phone-relay", "ble", "zepp-export"],
            ),
            privacy_map("local", 60_000),
            watch_rate_map(),
            streams_map(&[
                "heart-rate",
                "motion",
                "location",
                "environment",
                "battery",
                "connection",
            ]),
        ),
        "watch-sleep" => (
            watch_display_map("amazfit-t-rex-3-pro-44", 466, 44),
            input_map(&["button", "tap", "raise", "haptic-ack"]),
            output_map(&["screen", "haptic", "tone"]),
            transport_map_with_links(
                "phone-relay",
                250,
                true,
                &["modeled", "phone-relay", "zepp-export"],
            ),
            privacy_map("local", 60_000),
            watch_rate_map(),
            streams_map(&["heart-rate", "motion", "battery"]),
        ),
        "glasses" => (
            glasses_display_map(
                "generic-display",
                "mono",
                (1280, 720),
                40,
                "display-only",
                &[],
                &["screen"],
            ),
            input_map(&["button"]),
            output_map(&["screen"]),
            transport_map_with_links("usb", 20, false, &["usb"]),
            privacy_map("local", 60_000),
            rate_map(1, 1, 1000),
            streams_map(&[]),
        ),
        "glasses-hud" => (
            halo_display_map(&[]),
            halo_input_map(false),
            halo_output_map(),
            halo_transport_map(),
            privacy_map("local", 60_000),
            rate_map(5, 30, 200),
            streams_map(&["motion", "mic", "battery", "connection"]),
        ),
        "glasses-hud-camera" => (
            halo_display_map(&["camera"]),
            halo_input_map(true),
            halo_output_map(),
            halo_transport_map(),
            privacy_map("local", 60_000),
            rate_map(5, 30, 200),
            streams_map(&["motion", "camera", "mic", "battery", "connection"]),
        ),
        "glasses-3dof" => (
            viture_display_map("3dof"),
            input_map(&["head", "button"]),
            output_map(&["screen"]),
            transport_map_with_links("usb", 20, false, &["usb", "phone-relay"]),
            privacy_map("local", 60_000),
            rate_map(60, 120, 50),
            streams_map(&["motion"]),
        ),
        "glasses-stereo" => (
            viture_display_map("display-only"),
            input_map(&["button"]),
            output_map(&["screen"]),
            transport_map_with_links("usb", 20, false, &["usb"]),
            privacy_map("local", 60_000),
            rate_map(1, 1, 1000),
            streams_map(&[]),
        ),
        "glasses-luma-ultra" => (
            viture_display_map("6dof"),
            input_map(&["gaze", "head", "hand", "button"]),
            output_map(&["screen", "hud"]),
            transport_map_with_links("usb", 10, false, &["usb"]),
            privacy_map("local", 60_000),
            rate_map(60, 120, 25),
            streams_map(&[
                "pose",
                "motion",
                "camera",
                "depth-camera",
                "hand",
                "vio-status",
            ]),
        ),
        "phone" => (
            display_map(&[("density", sym("compact")), ("color", sym("truecolor"))]),
            input_map(&["touch", "voice", "camera"]),
            output_map(&["screen", "speaker", "mic"]),
            transport_map("relay", 120, true),
            privacy_map("session", 300_000),
            rate_map(5, 30, 500),
            streams_map(&["motion"]),
        ),
        "desktop" => (
            display_map(&[("density", sym("dense")), ("color", sym("truecolor"))]),
            input_map(&["keyboard", "pointer", "wheel", "file-drop"]),
            output_map(&["screen"]),
            transport_map("local", 1, false),
            privacy_map("session", 600_000),
            rate_map(60, 120, 100),
            streams_map(&[]),
        ),
        _ => return None,
    };
    Some(SurfaceCaps {
        client_id: name.to_owned(),
        preset: Symbol::qualified(SURFACE_NAMESPACE, name),
        display,
        input,
        output,
        transport,
        privacy,
        rate,
        streams,
        codecs: vec![
            Symbol::qualified(SURFACE_NAMESPACE, "lisp"),
            Symbol::qualified(SURFACE_NAMESPACE, "json"),
        ],
    })
}

use sim_value::build::sym;

fn display_map(extra: &[(&str, Expr)]) -> Expr {
    let mut entries: Vec<(&str, Expr)> = vec![("media", build::list(Vec::new()))];
    entries.extend(extra.iter().map(|(k, v)| (*k, v.clone())));
    build::map(entries)
}

fn input_map(flags: &[&str]) -> Expr {
    build::map(flags.iter().map(|flag| (*flag, Expr::Bool(true))).collect())
}

fn output_map(flags: &[&str]) -> Expr {
    input_map(flags)
}

fn streams_map(flags: &[&str]) -> Expr {
    input_map(flags)
}

fn watch_display_map(model: &str, px: u64, size_mm: u64) -> Expr {
    display_map(&[
        ("class", sym("watch")),
        ("shape", sym("round")),
        ("model", sym(model)),
        ("px", build::list(vec![build::uint(px), build::uint(px)])),
        ("size-mm", build::uint(size_mm)),
        ("color", sym("truecolor")),
        ("max-hz", build::uint(1)),
        ("density", sym("glance")),
    ])
}

fn watch_output_map() -> Expr {
    output_map(&["screen", "haptic", "face", "tone", "speaker", "mic"])
}

fn glasses_display_map(
    model: &str,
    display: &str,
    px: (u64, u64),
    fov_deg: u64,
    tracking: &str,
    cameras: &[&str],
    anchor_spaces: &[&str],
) -> Expr {
    let mut entries = vec![
        ("class", sym("glasses")),
        ("model", sym(model)),
        ("display", sym(display)),
        ("fov-deg", build::uint(fov_deg)),
        ("tracking-class", sym(tracking)),
        ("cameras", sym_list(cameras)),
        ("anchor-spaces", sym_list(anchor_spaces)),
        ("density", sym("glance")),
    ];
    match display {
        "mono" => {
            entries.push(("mono", Expr::Bool(true)));
            entries.push(("mono-px", px_pair(px.0, px.1)));
        }
        "stereo" => {
            entries.push(("stereo", Expr::Bool(true)));
            entries.push(("per-eye-px", px_pair(px.0, px.1)));
        }
        "none" => entries.push(("none", Expr::Bool(true))),
        _ => {}
    }
    display_map(&entries)
}

fn halo_display_map(cameras: &[&str]) -> Expr {
    let mut display = match glasses_display_map(
        "brilliant-halo",
        "mono",
        (256, 256),
        20,
        "hud",
        cameras,
        &["screen"],
    ) {
        Expr::Map(entries) => entries,
        _ => unreachable!(),
    };
    display.push((Expr::Symbol(build::keyword("lines")), build::uint(1)));
    Expr::Map(display)
}

fn halo_input_map(camera: bool) -> Expr {
    if camera {
        input_map(&["voice", "tap", "button", "camera", "haptic-ack"])
    } else {
        input_map(&["voice", "tap", "button", "haptic-ack"])
    }
}

fn halo_output_map() -> Expr {
    output_map(&["screen", "hud", "audio", "speaker", "haptic"])
}

fn halo_transport_map() -> Expr {
    transport_map_with_links(
        "bluetooth",
        80,
        true,
        &["bluetooth", "web-bluetooth", "phone-relay"],
    )
}

fn viture_display_map(tracking: &str) -> Expr {
    glasses_display_map(
        "viture-luma-ultra",
        "stereo",
        (1920, 1200),
        52,
        tracking,
        &["rgb-camera", "depth-camera"],
        &["head", "world", "hand"],
    )
}

fn px_pair(w: u64, h: u64) -> Expr {
    build::list(vec![build::uint(w), build::uint(h)])
}

fn sym_list(names: &[&str]) -> Expr {
    build::list(names.iter().map(|name| sym(name)).collect())
}

fn transport_map(kind: &str, round_trip_ms: u64, offline_queue: bool) -> Expr {
    transport_map_with_links(kind, round_trip_ms, offline_queue, &[])
}

fn transport_map_with_links(
    kind: &str,
    round_trip_ms: u64,
    offline_queue: bool,
    links: &[&str],
) -> Expr {
    build::map(vec![
        ("kind", build::sym(kind)),
        ("round-trip-ms", build::uint(round_trip_ms)),
        ("offline-queue", Expr::Bool(offline_queue)),
        ("ordered", Expr::Bool(true)),
        (
            "links",
            build::list(links.iter().map(|link| build::sym(link)).collect()),
        ),
    ])
}

fn privacy_map(class: &str, retain_ms: u64) -> Expr {
    build::map(vec![
        ("class", build::sym(class)),
        ("retain-ms", build::uint(retain_ms)),
        ("private-fields", build::list(Vec::new())),
    ])
}

fn rate_map(content_hz: u64, adapt_hz: u64, max_stale_ms: u64) -> Expr {
    build::map(vec![
        ("content-hz", build::uint(content_hz)),
        ("adapt-hz", build::uint(adapt_hz)),
        ("max-stale-ms", build::uint(max_stale_ms)),
    ])
}

fn watch_rate_map() -> Expr {
    rate_map(1, 1, 4000)
}

fn map_field(entries: &[(Expr, Expr)], name: &'static str) -> Result<Expr, SurfaceError> {
    // Defer the required-field lookup to the shared `sim_value::access` reader
    // (mapping its error via `SurfaceError::from`); keep the map-shape check
    // local, since the surface fields are whole `Expr::Map` values with no typed
    // slice reader of their own.
    match access::entry_required(entries, name, "surface caps").map_err(SurfaceError::from)? {
        value @ Expr::Map(_) => Ok(value.clone()),
        _ => Err(SurfaceError::BadField(name)),
    }
}

fn optional_map_field(entries: &[(Expr, Expr)], name: &'static str) -> Result<Expr, SurfaceError> {
    match access::entry_field(entries, name) {
        Some(value @ Expr::Map(_)) => Ok(value.clone()),
        Some(_) => Err(SurfaceError::BadField(name)),
        None => Ok(build::map(Vec::new())),
    }
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
