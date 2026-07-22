//! Watch-specific consent gates over the shared DEVICE_3 consent contract.

use sim_kernel::{CapabilityName, Cx, Error, Expr, Result, Symbol};
use sim_lib_view_device::{
    ConsentReceipt, DeviceSampleStore, EdgeId, Evicted, FrameClock, RetentionReaper, StoreKey,
    StoredSample, require_with_consent,
};
use sim_value::{access, build};

use crate::CAP_WATCH_MIC;

/// Capability required for watch health and biometric streams.
pub const CAP_WATCH_HEALTH: &str = "watch/health";

/// Capability required for watch location and route streams.
pub const CAP_WATCH_LOCATION: &str = "watch/location";

/// Capability required for watch vendor diagnostics.
pub const CAP_WATCH_VENDOR_REPORT: &str = "watch/vendor-report";

const WORN_SENSOR_NAMESPACE: &str = "stream/worn-sensor";
const WORN_SAMPLE_NAMESPACE: &str = "stream/device-sample";
const WORN_SAMPLE_KIND: &str = "worn-event";

/// Watch-sensitive capability classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchCapability {
    /// Heart-rate, SpO2, temperature, sleep, sport, and other health-style lanes.
    Health,
    /// GPS and route lanes.
    Location,
    /// Raw microphone audio lanes.
    Mic,
    /// Vendor diagnostics, off unless explicitly granted.
    VendorReport,
}

impl WatchCapability {
    /// All watch-sensitive capabilities.
    pub const ALL: [Self; 4] = [Self::Health, Self::Location, Self::Mic, Self::VendorReport];

    /// Stable kernel capability name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Health => CAP_WATCH_HEALTH,
            Self::Location => CAP_WATCH_LOCATION,
            Self::Mic => CAP_WATCH_MIC,
            Self::VendorReport => CAP_WATCH_VENDOR_REPORT,
        }
    }

    /// Stable local token after the `watch/` prefix.
    pub fn local_name(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Location => "location",
            Self::Mic => "mic",
            Self::VendorReport => "vendor-report",
        }
    }

    /// Kernel capability value.
    pub fn capability_name(self) -> CapabilityName {
        CapabilityName::new(self.as_str())
    }

    /// Visible grant symbol carried by a [`ConsentReceipt`].
    pub fn grant_symbol(self) -> Symbol {
        Symbol::qualified("watch", self.local_name())
    }

    /// Resolves a watch capability name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.as_str() == name)
    }
}

/// Returns the visible grant symbol for watch health streams.
pub fn watch_health_grant() -> Symbol {
    WatchCapability::Health.grant_symbol()
}

/// Returns the visible grant symbol for watch location streams.
pub fn watch_location_grant() -> Symbol {
    WatchCapability::Location.grant_symbol()
}

/// Returns the visible grant symbol for watch vendor diagnostics.
pub fn watch_vendor_report_grant() -> Symbol {
    WatchCapability::VendorReport.grant_symbol()
}

/// Classifies a strict worn-event expression into the watch capability it needs.
pub fn worn_event_capability(event: &Expr) -> Result<WatchCapability> {
    ensure_worn_event_sample(event)?;
    let sensor = access::required_sym(event, "sensor", "watch worn event")?;
    if sensor.namespace.as_deref() != Some(WORN_SENSOR_NAMESPACE) {
        return Err(Error::HostError(format!(
            "watch worn sensor must be in {WORN_SENSOR_NAMESPACE}, found {sensor}"
        )));
    }
    Ok(match sensor.name.as_ref() {
        "gps" | "route" => WatchCapability::Location,
        "mic-audio" => WatchCapability::Mic,
        _ => WatchCapability::Health,
    })
}

/// Requires the capability and visible consent needed by one worn-event expression.
pub fn require_worn_consent(
    cx: &Cx,
    event: &Expr,
    receipt: &ConsentReceipt,
    session: &EdgeId,
) -> Result<WatchCapability> {
    let capability = worn_event_capability(event)?;
    require_with_consent(cx, capability.as_str(), receipt, session)?;
    Ok(capability)
}

/// Accepts one worn event after enforcing a session-bound visible grant.
pub fn ingest_worn_expr(
    cx: &Cx,
    event: &Expr,
    receipt: &ConsentReceipt,
    session: &EdgeId,
) -> Result<Expr> {
    require_worn_consent(cx, event, receipt, session)?;
    Ok(event.clone())
}

/// Stores one sensitive worn event under the receipt sequence and modeled clock.
pub fn store_worn_sample(
    store: &mut DeviceSampleStore,
    event: &Expr,
    receipt: &ConsentReceipt,
    clock: FrameClock,
    content_refs: Vec<StoreKey>,
) -> Result<StoreKey> {
    let capability = worn_event_capability(event)?;
    let seq = event_seq(event)?;
    let key = StoreKey::new(Symbol::qualified(
        "watch/store",
        format!("{}-{seq}", capability.local_name()),
    ));
    let value = stored_worn_value(capability, event);
    store.insert_sample(StoredSample::new(
        key.clone(),
        receipt.seq,
        clock.tick,
        content_refs,
        value,
    ));
    Ok(key)
}

/// Runs the shared DEVICE_3 retention reaper for watch-sensitive samples.
pub fn sweep_watch_privacy(
    store: &mut DeviceSampleStore,
    receipts: &[ConsentReceipt],
    clock: FrameClock,
) -> Vec<Evicted> {
    RetentionReaper::new().sweep(store, receipts, clock)
}

/// Renders active watch grants and retention windows as a scene badge cluster.
pub fn active_watch_consent_badge_cluster(receipts: &[ConsentReceipt]) -> Expr {
    let mut badges = Vec::new();
    for receipt in receipts {
        for grant in &receipt.grants {
            if grant.namespace.as_deref() == Some("watch") {
                badges.push(sim_lib_scene::badge(
                    "ok",
                    &format!("{} {}", grant.as_qualified_str(), receipt.retain_ms),
                ));
            }
        }
        if !receipt.redact.is_empty() {
            badges.push(sim_lib_scene::badge(
                "warn",
                &format!("retention {}ms", receipt.retain_ms),
            ));
        }
    }
    sim_lib_scene::badge_cluster(badges)
}

fn ensure_worn_event_sample(event: &Expr) -> Result<()> {
    match access::field_sym(event, "sample") {
        Some(sample)
            if sample.namespace.as_deref() == Some(WORN_SAMPLE_NAMESPACE)
                && sample.name.as_ref() == WORN_SAMPLE_KIND =>
        {
            Ok(())
        }
        _ => Err(Error::HostError(
            "expected stream/device-sample worn-event".to_owned(),
        )),
    }
}

fn event_seq(event: &Expr) -> Result<u64> {
    let value = access::required(event, "seq", "watch worn event")?;
    match value {
        Expr::Number(number) if number.domain.namespace.is_none() => number
            .canonical
            .parse()
            .map_err(|_| Error::Eval("watch worn event seq is not u64".to_owned())),
        _ => Err(Error::TypeMismatch {
            expected: "u64 seq",
            found: "non-number",
        }),
    }
}

fn stored_worn_value(capability: WatchCapability, event: &Expr) -> Expr {
    Expr::Map(vec![
        (build::sym("kind"), build::qsym("watch", "worn-sample")),
        (
            build::sym("capability"),
            Expr::Symbol(capability.grant_symbol()),
        ),
        (
            Expr::Symbol(Symbol::new(capability.local_name())),
            event.clone(),
        ),
    ])
}
