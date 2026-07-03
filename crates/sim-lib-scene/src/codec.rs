//! `codec:scene`: the domain codec for Scene values.
//!
//! Modelled on `codec:chat`, this codec round-trips Scene values only and fails
//! closed outside its domain: it validates the scene on the way in and on the
//! way out, and the canonical text form (see [`crate::text`]) refuses non-data
//! `Expr` forms. A malformed scene becomes a structured `CodecError`, never a
//! panic. It is built on the shared [`DomainCodecLib`] scaffold and registers
//! the scene node Shapes through `with_shapes`.

use std::sync::Arc;

use sim_codec::{Decoder, DomainCodecLib, Encoder, Input, Output, ReadCx, domain_input_text};
use sim_kernel::{CodecId, Error, Lib, LibManifest, Linker, LoadCx, Result, Symbol, WriteCx};
use sim_shape::shape_value;

use crate::model::validate_scene;
use crate::shapes::{scene_shape_specs, scene_shape_symbol};
use crate::text;

/// Stable codec symbol for the scene domain codec.
pub fn scene_codec_symbol() -> Symbol {
    Symbol::qualified("codec", "scene")
}

/// The Scene domain codec object.
pub struct SceneCodec;

impl Decoder for SceneCodec {
    fn decode(&self, cx: &mut ReadCx<'_>, input: Input) -> Result<sim_kernel::Expr> {
        let source = domain_input_text(cx.codec, input)?;
        let expr = text::decode(cx.codec, &source)?;
        validate(cx.codec, &expr)?;
        Ok(expr)
    }
}

impl Encoder for SceneCodec {
    fn encode(&self, cx: &mut WriteCx<'_>, expr: &sim_kernel::Expr) -> Result<Output> {
        validate(cx.codec, expr)?;
        Ok(Output::Text(text::encode(cx.codec, expr)?))
    }
}

fn validate(codec: CodecId, expr: &sim_kernel::Expr) -> Result<()> {
    validate_scene(expr).map_err(|error| Error::CodecError {
        codec,
        message: format!("malformed scene at {error}"),
    })
}

/// Library that registers the scene node Shapes and `codec:scene`.
pub struct SceneCodecLib {
    symbol: Symbol,
    codec_id: CodecId,
}

impl SceneCodecLib {
    /// Build the lib with a freshly allocated codec id.
    pub fn new(codec_id: CodecId) -> Self {
        Self {
            symbol: scene_codec_symbol(),
            codec_id,
        }
    }

    fn domain_lib(&self) -> DomainCodecLib {
        let shapes = scene_shape_specs()
            .into_iter()
            .map(|(symbol, shape)| (symbol.clone(), shape_value(symbol, shape)))
            .collect();
        DomainCodecLib::new(
            self.symbol.clone(),
            self.codec_id,
            Arc::new(SceneCodec),
            Arc::new(SceneCodec),
            scene_shape_symbol(),
        )
        .with_shapes(shapes)
    }
}

impl Lib for SceneCodecLib {
    fn manifest(&self) -> LibManifest {
        self.domain_lib().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        self.domain_lib().load(cx, linker)
    }
}
