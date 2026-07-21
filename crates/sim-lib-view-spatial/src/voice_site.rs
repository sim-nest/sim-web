//! Consent-gated glasses voice capture through a placed ASR site.
//!
//! The Halo edge records microphone audio by reference as `xr/mic-chunk` data.
//! Recognition is an eval-fabric placement: local, phone-relay, or fabric. The
//! site returns the already-formed Intent; this module only enforces consent,
//! calls the site, and validates the Intent shape.

use sim_kernel::{
    CapabilityName, Consistency, Cx, Error, EvalFabric, EvalMode, EvalRequest, Expr, Result, Symbol,
};
use sim_lib_intent::validate_intent;
use sim_lib_view_device::{ConsentReceipt, EdgeId, require_with_consent};
use sim_value::{access, build};

/// Capability required for glasses microphone capture.
pub const CAP_GLASSES_MIC: &str = "glasses/mic";

/// Namespace for glasses microphone chunk references.
pub const XR_MIC_CHUNK_NAMESPACE: &str = "xr";

/// Kind tag for glasses microphone chunk references.
pub const XR_MIC_CHUNK_KIND: &str = "mic-chunk";

/// Where a glasses ASR site is placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsrSitePlacement {
    /// ASR runs in the local host process.
    Local,
    /// ASR is relayed through the paired phone.
    PhoneRelay,
    /// ASR is placed on a fabric site.
    Fabric,
}

impl AsrSitePlacement {
    /// Stable placement label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::PhoneRelay => "phone-relay",
            Self::Fabric => "fabric",
        }
    }

    /// Expression symbol naming this placement.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("asr/site-placement", self.as_str())
    }
}

/// A placed glasses ASR site.
pub struct AsrSite<'a> {
    placement: AsrSitePlacement,
    fabric: &'a dyn EvalFabric,
}

impl<'a> AsrSite<'a> {
    /// Creates a placed ASR site over an eval fabric.
    pub fn new(placement: AsrSitePlacement, fabric: &'a dyn EvalFabric) -> Self {
        Self { placement, fabric }
    }

    /// Creates a local ASR site.
    pub fn local(fabric: &'a dyn EvalFabric) -> Self {
        Self::new(AsrSitePlacement::Local, fabric)
    }

    /// Creates a phone-relay ASR site.
    pub fn phone_relay(fabric: &'a dyn EvalFabric) -> Self {
        Self::new(AsrSitePlacement::PhoneRelay, fabric)
    }

    /// Creates a fabric-placed ASR site.
    pub fn fabric(fabric: &'a dyn EvalFabric) -> Self {
        Self::new(AsrSitePlacement::Fabric, fabric)
    }

    /// Returns this site's placement.
    pub fn placement(&self) -> AsrSitePlacement {
        self.placement
    }

    fn realize(&self, cx: &mut Cx, chunk: &XrMicChunkRef) -> Result<Expr> {
        let reply = self.fabric.realize(
            cx,
            EvalRequest {
                expr: chunk.to_expr(),
                result_shape: None,
                required_capabilities: vec![glasses_mic_capability()],
                deadline: None,
                consistency: Consistency::LocalFirst,
                mode: EvalMode::Eval,
                answer_limit: None,
                stream_buffer: None,
                stream: false,
                trace: false,
            },
        )?;
        reply.value.object().as_expr(cx)
    }
}

/// A by-reference glasses microphone chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XrMicChunkRef {
    /// Store key or stream key for the referenced audio chunk.
    pub ref_id: Symbol,
    /// Monotonic capture sequence.
    pub seq: u64,
    /// PCM sample rate.
    pub sample_rate_hz: u32,
    /// Number of PCM channels.
    pub channels: u8,
    /// Referenced audio byte length.
    pub byte_len: u64,
}

impl XrMicChunkRef {
    /// Builds a microphone chunk reference.
    pub fn new(
        ref_id: Symbol,
        seq: u64,
        sample_rate_hz: u32,
        channels: u8,
        byte_len: u64,
    ) -> Result<Self> {
        if sample_rate_hz == 0 || channels == 0 {
            return Err(Error::HostError(
                "xr mic chunk requires a nonzero PCM format".to_owned(),
            ));
        }
        if byte_len == 0 {
            return Err(Error::HostError(
                "xr mic chunk requires referenced audio bytes".to_owned(),
            ));
        }
        Ok(Self {
            ref_id,
            seq,
            sample_rate_hz,
            channels,
            byte_len,
        })
    }

    /// Encodes this reference as expression data.
    pub fn to_expr(&self) -> Expr {
        build::map(vec![
            (
                "kind",
                build::qsym(XR_MIC_CHUNK_NAMESPACE, XR_MIC_CHUNK_KIND),
            ),
            ("ref", Expr::Symbol(self.ref_id.clone())),
            ("seq", build::uint(self.seq)),
            (
                "sample-rate-hz",
                build::uint(u64::from(self.sample_rate_hz)),
            ),
            ("channels", build::uint(u64::from(self.channels))),
            ("bytes", build::uint(self.byte_len)),
        ])
    }

    /// Decodes a microphone chunk reference, rejecting embedded audio or text.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        ensure_kind(expr)?;
        ensure_no_extra(
            expr,
            &["kind", "ref", "seq", "sample-rate-hz", "channels", "bytes"],
            "xr mic chunk",
        )?;
        let ref_id = match access::required(expr, "ref", "xr mic chunk")? {
            Expr::Symbol(symbol) => symbol.clone(),
            _ => {
                return Err(Error::TypeMismatch {
                    expected: "audio chunk reference symbol",
                    found: "non-symbol",
                });
            }
        };
        Self::new(
            ref_id,
            uint_field(expr, "seq", "xr mic chunk")?,
            u32_field(expr, "sample-rate-hz", "xr mic chunk")?,
            u8_field(expr, "channels", "xr mic chunk")?,
            uint_field(expr, "bytes", "xr mic chunk")?,
        )
    }
}

/// Returns the kernel capability name for glasses microphone capture.
pub fn glasses_mic_capability() -> CapabilityName {
    CapabilityName::new(CAP_GLASSES_MIC)
}

/// Returns the visible consent grant symbol for glasses microphone capture.
pub fn glasses_mic_grant() -> Symbol {
    Symbol::qualified("glasses", "mic")
}

/// Produces a voice Intent through a placed ASR site.
///
/// This function first enforces `glasses/mic` through kernel capability state
/// and the session-bound visible consent receipt. It then realizes the placed
/// site with an `xr/mic-chunk` reference. The site output must already validate
/// as a standard `intent/*` value.
pub fn voice_intent_via_site(
    cx: &mut Cx,
    chunk_ref: &XrMicChunkRef,
    site: Option<&AsrSite<'_>>,
    receipt: &ConsentReceipt,
    session: &EdgeId,
) -> Result<Expr> {
    require_with_consent(cx, CAP_GLASSES_MIC, receipt, session)?;
    let site = site.ok_or_else(|| {
        Error::HostError("glasses voice unavailable: no ASR site placed".to_owned())
    })?;
    let intent = site.realize(cx, chunk_ref)?;
    validate_intent(&intent)
        .map_err(|err| Error::HostError(format!("ASR site output is not a voice Intent: {err}")))?;
    Ok(intent)
}

fn ensure_kind(expr: &Expr) -> Result<()> {
    match access::field_sym(expr, "kind") {
        Some(symbol)
            if symbol.namespace.as_deref() == Some(XR_MIC_CHUNK_NAMESPACE)
                && symbol.name.as_ref() == XR_MIC_CHUNK_KIND =>
        {
            Ok(())
        }
        _ => Err(Error::HostError("expected xr mic chunk".to_owned())),
    }
}

fn ensure_no_extra(expr: &Expr, allowed: &[&str], context: &str) -> Result<()> {
    let Expr::Map(entries) = expr else {
        return Err(Error::HostError(format!("expected {context}")));
    };
    for (key, _) in entries {
        let Expr::Symbol(symbol) = key else {
            return Err(Error::HostError(format!(
                "{context} has a non-symbol field"
            )));
        };
        if symbol.namespace.is_some() || !allowed.contains(&symbol.name.as_ref()) {
            return Err(Error::HostError(format!(
                "{context} has unexpected field {}",
                symbol.as_qualified_str()
            )));
        }
    }
    Ok(())
}

fn uint_field(expr: &Expr, name: &str, context: &str) -> Result<u64> {
    match access::required(expr, name, context)? {
        Expr::Number(number) if number.domain.namespace.is_none() => number
            .canonical
            .parse()
            .map_err(|_| Error::Eval(format!("{context} field {name} is not u64"))),
        _ => Err(Error::Eval(format!("{context} field {name} is not u64"))),
    }
}

fn u32_field(expr: &Expr, name: &str, context: &str) -> Result<u32> {
    uint_field(expr, name, context).and_then(|value| {
        u32::try_from(value).map_err(|_| Error::Eval(format!("{context} field {name} exceeds u32")))
    })
}

fn u8_field(expr: &Expr, name: &str, context: &str) -> Result<u8> {
    uint_field(expr, name, context).and_then(|value| {
        u8::try_from(value).map_err(|_| Error::Eval(format!("{context} field {name} exceeds u8")))
    })
}
