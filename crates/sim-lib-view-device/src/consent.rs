//! Session-bound device consent receipts.

use sim_kernel::{CapabilityName, Cx, Error, EventKind, EventLedger, Expr, Ref, Result, Symbol};
use sim_lib_scene::{GlanceCard, GlanceMetric};
use sim_value::{access, build};

/// Capability helpers for device-local sensors and actuators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceCapability {
    /// Pose or spatial tracking samples.
    Pose,
    /// Camera frames or still captures.
    Camera,
    /// Health or biometric samples.
    Health,
    /// Location samples.
    Location,
    /// Microphone input samples.
    Mic,
    /// Vendor diagnostic reports.
    VendorReport,
}

impl DeviceCapability {
    /// All baseline device capabilities.
    pub const ALL: [Self; 6] = [
        Self::Pose,
        Self::Camera,
        Self::Health,
        Self::Location,
        Self::Mic,
        Self::VendorReport,
    ];

    /// Stable capability name, suitable for [`Cx::require`].
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pose => "device/pose",
            Self::Camera => "device/camera",
            Self::Health => "device/health",
            Self::Location => "device/location",
            Self::Mic => "device/mic",
            Self::VendorReport => "device/vendor-report",
        }
    }

    /// Returns the kernel capability name.
    pub fn capability_name(self) -> CapabilityName {
        CapabilityName::new(self.as_str())
    }

    /// Returns the visible consent grant symbol.
    pub fn grant_symbol(self) -> Symbol {
        Symbol::qualified("device", self.local_name())
    }

    /// Resolves a baseline helper from a capability name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.as_str() == name)
    }

    fn local_name(self) -> &'static str {
        self.as_str()
            .strip_prefix("device/")
            .expect("device capability names keep the device/ prefix")
    }
}

/// Stable device-edge session identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeId(Symbol);

impl EdgeId {
    /// Builds an edge id from a stable symbol.
    pub fn new(symbol: Symbol) -> Self {
        Self(symbol)
    }

    /// Builds an edge id in the `device/session` namespace.
    pub fn named(name: impl Into<String>) -> Self {
        Self(Symbol::qualified("device/session", name.into()))
    }

    /// Returns the backing symbol.
    pub fn as_symbol(&self) -> &Symbol {
        &self.0
    }

    /// Encodes this id as expression data.
    pub fn to_expr(&self) -> Expr {
        Expr::Symbol(self.0.clone())
    }

    /// Decodes an edge id from expression data.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        match expr {
            Expr::Symbol(symbol) => Ok(Self(symbol.clone())),
            _ => Err(Error::TypeMismatch {
                expected: "device edge id symbol",
                found: "non-symbol",
            }),
        }
    }
}

impl From<Symbol> for EdgeId {
    fn from(symbol: Symbol) -> Self {
        Self::new(symbol)
    }
}

/// A visible consent receipt bound to one device-edge session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsentReceipt {
    /// Granted device capabilities, as visible `device/*` symbols.
    pub grants: Vec<Symbol>,
    /// Modeled retention window in milliseconds.
    pub retain_ms: u64,
    /// Fields or streams that must be redacted by downstream reapers.
    pub redact: Vec<Symbol>,
    /// Session that owns the visible consent.
    pub session: EdgeId,
    /// Ledger sequence number.
    pub seq: u64,
}

impl ConsentReceipt {
    /// Builds a receipt with an explicit ledger sequence.
    pub fn new(
        grants: Vec<Symbol>,
        retain_ms: u64,
        redact: Vec<Symbol>,
        session: EdgeId,
        seq: u64,
    ) -> Self {
        Self {
            grants: dedup_symbols(grants),
            retain_ms,
            redact: dedup_symbols(redact),
            session,
            seq,
        }
    }

    /// Encodes the receipt as ordinary expression data.
    pub fn to_expr(&self) -> Expr {
        build::map(vec![
            ("kind", build::qsym("device", "consent-receipt")),
            ("grants", symbol_list(&self.grants)),
            ("retain-ms", build::uint(self.retain_ms)),
            ("redact", symbol_list(&self.redact)),
            ("session", self.session.to_expr()),
            ("seq", build::uint(self.seq)),
        ])
    }

    /// Decodes a receipt from expression data.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        ensure_kind(expr)?;
        let grants = symbol_vec(access::required(expr, "grants", "device consent receipt")?)?;
        let retain_ms = uint_field(expr, "retain-ms")?;
        let redact = symbol_vec(access::required(expr, "redact", "device consent receipt")?)?;
        let session =
            EdgeId::from_expr(access::required(expr, "session", "device consent receipt")?)?;
        let seq = uint_field(expr, "seq")?;
        Ok(Self::new(grants, retain_ms, redact, session, seq))
    }

    /// Renders the receipt as a compact scene badge for the active session.
    pub fn to_badge_scene(&self) -> Expr {
        sim_lib_scene::badge("ok", &format!("consent {}", self.seq))
    }

    /// Renders the receipt as a `scene/glance` card.
    pub fn to_glance_scene(&self) -> Expr {
        GlanceCard::new(
            "Device consent",
            Some(GlanceMetric::new(
                "session",
                self.session.as_symbol().as_qualified_str(),
            )),
            None,
            "info",
            1,
        )
        .to_scene()
    }
}

/// Records a consent receipt in a kernel event ledger and returns it with the
/// assigned sequence.
pub fn record_consent_receipt(
    ledger: &mut EventLedger,
    run: Ref,
    grants: Vec<Symbol>,
    retain_ms: u64,
    redact: Vec<Symbol>,
    session: EdgeId,
) -> Result<ConsentReceipt> {
    let next_seq = ledger.len_for_run(&run) as u64;
    let event = ledger.push(
        run,
        EventKind::Card {
            subject: Ref::Symbol(session.as_symbol().clone()),
            card: Ref::Symbol(Symbol::qualified(
                "device/consent",
                format!("receipt-{next_seq}"),
            )),
        },
    )?;
    Ok(ConsentReceipt::new(
        grants, retain_ms, redact, session, event.seq,
    ))
}

/// Requires both a kernel grant and a session-bound visible consent receipt.
pub fn require_with_consent(
    cx: &Cx,
    name: &str,
    receipt: &ConsentReceipt,
    session: &EdgeId,
) -> Result<()> {
    cx.require(&CapabilityName::new(name.to_owned()))?;
    if &receipt.session != session {
        return Err(Error::HostError(format!(
            "{name}: consent not for this session"
        )));
    }
    if !receipt
        .grants
        .iter()
        .any(|grant| grant_matches(grant, name))
    {
        return Err(Error::HostError(format!("{name} requires visible consent")));
    }
    Ok(())
}

fn ensure_kind(expr: &Expr) -> Result<()> {
    match access::field_sym(expr, "kind") {
        Some(kind)
            if kind.namespace.as_deref() == Some("device")
                && kind.name.as_ref() == "consent-receipt" =>
        {
            Ok(())
        }
        _ => Err(Error::HostError(
            "expected device/consent-receipt".to_owned(),
        )),
    }
}

fn uint_field(expr: &Expr, name: &str) -> Result<u64> {
    let value = access::required(expr, name, "device consent receipt")?;
    match value {
        Expr::Number(number) if number.domain.namespace.is_none() => number
            .canonical
            .parse()
            .map_err(|_| Error::Eval(format!("device consent receipt field {name} is not u64"))),
        _ => Err(Error::Eval(format!(
            "device consent receipt field {name} is not u64"
        ))),
    }
}

fn symbol_vec(expr: &Expr) -> Result<Vec<Symbol>> {
    match expr {
        Expr::List(items) => items
            .iter()
            .map(|item| match item {
                Expr::Symbol(symbol) => Ok(symbol.clone()),
                _ => Err(Error::TypeMismatch {
                    expected: "symbol list",
                    found: "non-symbol",
                }),
            })
            .collect(),
        _ => Err(Error::TypeMismatch {
            expected: "symbol list",
            found: "non-list",
        }),
    }
}

fn symbol_list(symbols: &[Symbol]) -> Expr {
    build::list(symbols.iter().cloned().map(Expr::Symbol).collect())
}

fn grant_matches(grant: &Symbol, name: &str) -> bool {
    grant.as_qualified_str() == name
}

fn dedup_symbols(mut symbols: Vec<Symbol>) -> Vec<Symbol> {
    symbols.sort();
    symbols.dedup();
    symbols
}
