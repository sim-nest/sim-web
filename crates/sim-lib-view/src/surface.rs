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
            display_map(&[("density", sym("glance")), ("lines", build::uint(2))]),
            input_map(&["voice", "tap"]),
            output_map(&["screen", "hud", "speaker"]),
            transport_map("relay", 250, true),
            privacy_map("local", 60_000),
            rate_map(5, 30, 500),
            streams_map(&["pose", "motion"]),
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
mod tests {
    use super::*;

    #[test]
    fn every_preset_round_trips() {
        for name in SURFACE_PRESETS {
            let caps = preset(name).expect("preset exists");
            assert_eq!(caps.preset_name(), *name);
            let back = SurfaceCaps::from_expr(&caps.to_expr()).expect("round-trips");
            assert_eq!(caps, back, "{name} caps must round-trip losslessly");
        }
    }

    #[test]
    fn unknown_preset_is_none() {
        assert!(preset("hologram").is_none());
    }

    #[test]
    fn from_preset_overrides_client_id() {
        let caps = SurfaceCaps::from_preset("cli", "tty.local.7").unwrap();
        assert_eq!(caps.client_id, "tty.local.7");
        assert_eq!(caps.preset_name(), "cli");
    }

    #[test]
    fn capability_accessors_read_fields() {
        let cli = preset("cli").unwrap();
        assert!(cli.input_flag("keyboard"));
        assert!(!cli.input_flag("touch"));
        assert_eq!(cli.display_density().unwrap().name.as_ref(), "dense");
        assert!(cli.accepts_codec("lisp"));
        assert!(!cli.accepts_codec("algol"));

        let watch = preset("watch").unwrap();
        assert!(watch.input_flag("haptic-ack"));
        assert_eq!(watch.display_density().unwrap().name.as_ref(), "glance");
    }

    #[test]
    fn surface_map_field_wrong_shape_fails_closed() {
        // A caps map whose `display` field is not a map must fail closed with a
        // located `BadField`, never partial caps.
        let mut entries = match preset("cli").unwrap().to_expr() {
            Expr::Map(entries) => entries,
            _ => unreachable!(),
        };
        for (key, value) in entries.iter_mut() {
            if matches!(key, Expr::Symbol(symbol) if &*symbol.name == "display") {
                *value = Expr::Bool(true);
            }
        }
        assert_eq!(
            SurfaceCaps::from_expr(&Expr::Map(entries)),
            Err(SurfaceError::BadField("display"))
        );
    }

    #[test]
    fn surface_map_field_missing_flows_through_sim_value_reader() {
        // A missing map field is reported by the shared `sim_value::access`
        // reader, adopted as `SurfaceError::Field` via `From<sim_value::Error>`.
        let mut entries = match preset("cli").unwrap().to_expr() {
            Expr::Map(entries) => entries,
            _ => unreachable!(),
        };
        entries.retain(|(key, _)| !matches!(key, Expr::Symbol(s) if &*s.name == "transport"));
        match SurfaceCaps::from_expr(&Expr::Map(entries)) {
            Err(SurfaceError::Field(message)) => assert!(message.contains("transport")),
            other => panic!("expected a located field error, got {other:?}"),
        }
    }

    #[test]
    fn parse_fails_closed() {
        assert_eq!(
            SurfaceCaps::from_expr(&Expr::Nil),
            Err(SurfaceError::NotCaps)
        );
        // A caps map missing `codecs` must not yield partial caps.
        let mut entries = match preset("cli").unwrap().to_expr() {
            Expr::Map(entries) => entries,
            _ => unreachable!(),
        };
        entries.retain(|(key, _)| !matches!(key, Expr::Symbol(s) if &*s.name == "codecs"));
        assert_eq!(
            SurfaceCaps::from_expr(&Expr::Map(entries)),
            Err(SurfaceError::MissingField("codecs"))
        );
    }

    #[test]
    fn missing_rate_map_defaults_to_safe_envelope() {
        let mut entries = match preset("cli").unwrap().to_expr() {
            Expr::Map(entries) => entries,
            _ => unreachable!(),
        };
        entries.retain(|(key, _)| !matches!(key, Expr::Symbol(s) if &*s.name == "rate"));
        let caps = SurfaceCaps::from_expr(&Expr::Map(entries)).expect("older caps parse");
        assert_eq!(caps.rate, rate_map(1, 1, 1000));
    }

    #[test]
    fn missing_output_and_stream_maps_default_to_empty_metadata() {
        let mut entries = match preset("watch-glance-large").unwrap().to_expr() {
            Expr::Map(entries) => entries,
            _ => unreachable!(),
        };
        entries.retain(|(key, _)| {
            !matches!(key, Expr::Symbol(s) if matches!(s.name.as_ref(), "output" | "streams"))
        });

        let caps = SurfaceCaps::from_expr(&Expr::Map(entries)).expect("older caps parse");

        assert_eq!(caps.output, build::map(Vec::new()));
        assert_eq!(caps.streams, build::map(Vec::new()));
    }
}
