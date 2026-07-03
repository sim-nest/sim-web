//! `codec:intent`: the domain codec for Intent values.
//!
//! Like `codec:scene`, this codec round-trips Intent values only and fails
//! closed outside its domain: it validates the Intent on the way in and out and
//! serializes through the codec-neutral portable value form in `sim-codec`. A
//! malformed Intent becomes a structured `CodecError`, never a panic. It is
//! built on the shared [`DomainCodecLib`] scaffold and registers the Intent kind
//! Shapes through `with_shapes`.

use std::sync::Arc;

use sim_codec::{
    Decoder, DomainCodecLib, Encoder, Input, Output, ReadCx, decode_portable, domain_input_text,
    encode_portable,
};
use sim_kernel::{CodecId, Error, Lib, LibManifest, Linker, LoadCx, Result, Symbol, WriteCx};
use sim_shape::shape_value;

use crate::model::validate_intent;
use crate::shapes::{intent_shape_specs, intent_shape_symbol};

/// Stable codec symbol for the intent domain codec.
pub fn intent_codec_symbol() -> Symbol {
    Symbol::qualified("codec", "intent")
}

/// The Intent domain codec object.
pub struct IntentCodec;

impl Decoder for IntentCodec {
    fn decode(&self, cx: &mut ReadCx<'_>, input: Input) -> Result<sim_kernel::Expr> {
        let source = domain_input_text(cx.codec, input)?;
        let expr = decode_portable(cx.codec, &source)?;
        validate(cx.codec, &expr)?;
        Ok(expr)
    }
}

impl Encoder for IntentCodec {
    fn encode(&self, cx: &mut WriteCx<'_>, expr: &sim_kernel::Expr) -> Result<Output> {
        validate(cx.codec, expr)?;
        Ok(Output::Text(encode_portable(cx.codec, expr)?))
    }
}

fn validate(codec: CodecId, expr: &sim_kernel::Expr) -> Result<()> {
    validate_intent(expr).map_err(|error| Error::CodecError {
        codec,
        message: format!("malformed intent at {error}"),
    })
}

/// Library that registers the Intent kind Shapes and `codec:intent`.
pub struct IntentCodecLib {
    symbol: Symbol,
    codec_id: CodecId,
}

impl IntentCodecLib {
    /// Build the lib with a freshly allocated codec id.
    pub fn new(codec_id: CodecId) -> Self {
        Self {
            symbol: intent_codec_symbol(),
            codec_id,
        }
    }

    fn domain_lib(&self) -> DomainCodecLib {
        let shapes = intent_shape_specs()
            .into_iter()
            .map(|(symbol, shape)| (symbol.clone(), shape_value(symbol, shape)))
            .collect();
        DomainCodecLib::new(
            self.symbol.clone(),
            self.codec_id,
            Arc::new(IntentCodec),
            Arc::new(IntentCodec),
            intent_shape_symbol(),
        )
        .with_shapes(shapes)
    }
}

impl Lib for IntentCodecLib {
    fn manifest(&self) -> LibManifest {
        self.domain_lib().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        self.domain_lib().load(cx, linker)
    }
}
