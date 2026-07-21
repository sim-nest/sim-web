//! Consent-gated watch microphone capture and ASR-site transcription.
//!
//! The watch side stores only raw framed PCM. Speech recognition is reached
//! through the location-transparent [`sim_kernel::EvalFabric`] contract and the
//! returned transcript is immediately wrapped as a normal Intent value.

use sim_kernel::{
    CapabilityName, Consistency, Cx, Error, EvalFabric, EvalMode, EvalRequest, Expr, Result, Symbol,
};
use sim_lib_intent::{Origin, intent};
use sim_lib_view_device::{ConsentReceipt, EdgeId, require_with_consent};
use sim_value::{access, build};

/// Capability required for watch microphone capture.
pub const CAP_WATCH_MIC: &str = "watch/mic";

/// Maximum frames retained in one watch microphone capture.
pub const MAX_MIC_FRAMES: usize = 64;

/// Maximum bytes accepted in one raw PCM frame.
pub const MAX_MIC_FRAME_BYTES: usize = 8192;

const MIC_CAPTURE_NS: &str = "watch";
const MIC_CAPTURE_KIND: &str = "mic-capture";
const AUDIO_FRAME_KIND: &str = "audio-frame";

/// One raw PCM frame captured from a watch microphone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioFrame {
    /// Monotonic frame timestamp in milliseconds.
    pub at_ms: u64,
    /// Raw PCM frame bytes.
    pub pcm: Vec<u8>,
}

impl AudioFrame {
    /// Builds one bounded raw PCM frame.
    pub fn new(at_ms: u64, pcm: Vec<u8>) -> Result<Self> {
        if pcm.len() > MAX_MIC_FRAME_BYTES {
            return Err(Error::HostError(format!(
                "watch mic frame exceeds {MAX_MIC_FRAME_BYTES} bytes"
            )));
        }
        Ok(Self { at_ms, pcm })
    }

    /// Encodes this raw frame as expression data.
    pub fn to_expr(&self) -> Expr {
        build::map(vec![
            ("kind", build::qsym(MIC_CAPTURE_NS, AUDIO_FRAME_KIND)),
            ("at-ms", build::uint(self.at_ms)),
            ("pcm", Expr::Bytes(self.pcm.clone())),
        ])
    }

    /// Decodes one raw frame, rejecting transcript-like side channels.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        ensure_kind(expr, AUDIO_FRAME_KIND, "watch audio frame")?;
        ensure_no_extra(expr, &["kind", "at-ms", "pcm"], "watch audio frame")?;
        let at_ms = uint_field(expr, "at-ms", "watch audio frame")?;
        let pcm = match access::required(expr, "pcm", "watch audio frame")? {
            Expr::Bytes(bytes) => bytes.clone(),
            _ => {
                return Err(Error::TypeMismatch {
                    expected: "raw PCM bytes",
                    found: "non-bytes",
                });
            }
        };
        Self::new(at_ms, pcm)
    }
}

/// Raw bounded watch microphone capture.
///
/// This is audio input, not text. It intentionally has no transcript field; ASR
/// output exists only as an eval-fabric reply passed to [`transcribe_via_site`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MicCapture {
    /// Ordered raw PCM frames.
    pub frames: Vec<AudioFrame>,
    /// Monotonic capture sequence.
    pub seq: u64,
    /// PCM sample rate.
    pub sample_rate_hz: u32,
    /// Number of PCM channels.
    pub channels: u8,
}

impl MicCapture {
    /// Builds a bounded microphone capture from raw frames.
    pub fn new(
        seq: u64,
        sample_rate_hz: u32,
        channels: u8,
        frames: Vec<AudioFrame>,
    ) -> Result<Self> {
        if frames.is_empty() {
            return Err(Error::HostError(
                "watch mic capture requires at least one frame".to_owned(),
            ));
        }
        if frames.len() > MAX_MIC_FRAMES {
            return Err(Error::HostError(format!(
                "watch mic capture exceeds {MAX_MIC_FRAMES} frames"
            )));
        }
        if sample_rate_hz == 0 || channels == 0 {
            return Err(Error::HostError(
                "watch mic capture requires a nonzero PCM format".to_owned(),
            ));
        }
        Ok(Self {
            frames,
            seq,
            sample_rate_hz,
            channels,
        })
    }

    /// Encodes this capture as expression data.
    pub fn to_expr(&self) -> Expr {
        build::map(vec![
            ("kind", build::qsym(MIC_CAPTURE_NS, MIC_CAPTURE_KIND)),
            (
                "frames",
                build::list(self.frames.iter().map(AudioFrame::to_expr).collect()),
            ),
            ("seq", build::uint(self.seq)),
            (
                "sample-rate-hz",
                build::uint(u64::from(self.sample_rate_hz)),
            ),
            ("channels", build::uint(u64::from(self.channels))),
        ])
    }

    /// Decodes a capture, rejecting embedded transcript fields.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        ensure_kind(expr, MIC_CAPTURE_KIND, "watch mic capture")?;
        ensure_no_extra(
            expr,
            &["kind", "frames", "seq", "sample-rate-hz", "channels"],
            "watch mic capture",
        )?;
        let frames = match access::required(expr, "frames", "watch mic capture")? {
            Expr::List(items) => items
                .iter()
                .map(AudioFrame::from_expr)
                .collect::<Result<Vec<_>>>()?,
            _ => {
                return Err(Error::TypeMismatch {
                    expected: "audio frame list",
                    found: "non-list",
                });
            }
        };
        let seq = uint_field(expr, "seq", "watch mic capture")?;
        let sample_rate_hz = u32_field(expr, "sample-rate-hz", "watch mic capture")?;
        let channels = u8_field(expr, "channels", "watch mic capture")?;
        Self::new(seq, sample_rate_hz, channels, frames)
    }
}

/// Returns the kernel capability name for watch microphone capture.
pub fn watch_mic_capability() -> CapabilityName {
    CapabilityName::new(CAP_WATCH_MIC)
}

/// Returns the visible consent grant symbol for watch microphone capture.
pub fn watch_mic_grant() -> Symbol {
    Symbol::qualified("watch", "mic")
}

/// Transcribes raw watch microphone frames through an ASR eval site.
///
/// This function first enforces `watch/mic` through kernel capability state and
/// the session-bound visible consent receipt. Only then does it call
/// [`EvalFabric::realize`] with the raw audio expression. The returned transcript
/// becomes an ordinary `intent/invoke`; callers never receive a free transcript
/// string from the watch input path.
pub fn transcribe_via_site(
    cx: &mut Cx,
    mic: &MicCapture,
    site: &dyn EvalFabric,
    receipt: &ConsentReceipt,
    session: &EdgeId,
    origin: Origin,
    target: Expr,
) -> Result<Expr> {
    require_with_consent(cx, CAP_WATCH_MIC, receipt, session)?;
    let reply = site.realize(
        cx,
        EvalRequest {
            expr: mic.to_expr(),
            result_shape: None,
            required_capabilities: vec![watch_mic_capability()],
            deadline: None,
            consistency: Consistency::LocalFirst,
            mode: EvalMode::Eval,
            answer_limit: None,
            stream_buffer: None,
            stream: false,
            trace: false,
        },
    )?;
    let transcript_expr = reply.value.object().as_expr(cx)?;
    let transcript = transcript_text(&transcript_expr)?;
    Ok(voice_intent(origin, target, transcript))
}

fn voice_intent(origin: Origin, target: Expr, transcript: String) -> Expr {
    intent(
        "invoke",
        origin,
        vec![
            ("target", target),
            (
                "op",
                Expr::Symbol(Symbol::qualified("watch/voice", "transcript")),
            ),
            ("args", Expr::List(vec![Expr::String(transcript)])),
        ],
    )
}

fn transcript_text(expr: &Expr) -> Result<String> {
    match expr {
        Expr::String(text) => Ok(text.clone()),
        Expr::Map(_) => match access::field(expr, "text") {
            Some(Expr::String(text)) => Ok(text.clone()),
            Some(_) => Err(Error::TypeMismatch {
                expected: "ASR transcript text",
                found: "non-string",
            }),
            None => Err(Error::HostError(
                "ASR site output is missing transcript text".to_owned(),
            )),
        },
        _ => Err(Error::TypeMismatch {
            expected: "ASR transcript output",
            found: "non-transcript",
        }),
    }
}

fn ensure_kind(expr: &Expr, kind: &str, context: &str) -> Result<()> {
    match access::field_sym(expr, "kind") {
        Some(symbol)
            if symbol.namespace.as_deref() == Some(MIC_CAPTURE_NS)
                && symbol.name.as_ref() == kind =>
        {
            Ok(())
        }
        _ => Err(Error::HostError(format!("expected {context}"))),
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
    let value = uint_field(expr, name, context)?;
    u32::try_from(value).map_err(|_| Error::Eval(format!("{context} field {name} is not u32")))
}

fn u8_field(expr: &Expr, name: &str, context: &str) -> Result<u8> {
    let value = uint_field(expr, name, context)?;
    u8::try_from(value).map_err(|_| Error::Eval(format!("{context} field {name} is not u8")))
}
