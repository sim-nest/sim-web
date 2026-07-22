//! Content-rate encoding plus device-rate local adaptation.

use std::rc::Rc;

use sim_kernel::{Cx, Expr, Result};
use sim_lib_view::{SurfaceCaps, SurfaceCodec};

use crate::{DeviceProfile, EncodedScene, LocalAdapter};

/// Runs one content encode and then many local adaptations.
///
/// The helper keeps device state out of the content encoder API. State is only
/// accepted by [`Split::adapt_many`] and the free [`drive`] function, both of
/// which operate on an already encoded [`EncodedScene`].
#[derive(Clone, Debug)]
pub struct Split<A> {
    adapter: A,
    profile: DeviceProfile,
}

impl<A> Split<A> {
    /// Builds a split driver for one local adapter and device profile.
    pub fn new(adapter: A, profile: DeviceProfile) -> Self {
        Self { adapter, profile }
    }

    /// Returns the local adapter.
    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    /// Returns the device profile used for local adaptations.
    pub fn profile(&self) -> &DeviceProfile {
        &self.profile
    }
}

impl<A: LocalAdapter> Split<A> {
    /// Encodes content once through a [`SurfaceCodec`].
    pub fn encode_once<C: SurfaceCodec + ?Sized>(
        &self,
        codec: &C,
        cx: &mut Cx,
        value: &Expr,
        caps: &SurfaceCaps,
    ) -> Result<EncodedScene> {
        codec.encode(cx, value, caps).map(EncodedScene::new)
    }

    /// Adapts one already encoded Scene using the latest device state.
    pub fn adapt_one(&self, encoded: &EncodedScene, state: &A::State) -> Result<Rc<Expr>> {
        self.adapter.adapt(encoded, state, &self.profile)
    }

    /// Adapts one already encoded Scene for each device state.
    pub fn adapt_many(&self, encoded: &EncodedScene, states: &[A::State]) -> Result<Vec<Rc<Expr>>> {
        drive(encoded, &self.adapter, states, &self.profile)
    }

    /// Runs the full split: one encode followed by local adaptations.
    pub fn run<C: SurfaceCodec + ?Sized>(
        &self,
        codec: &C,
        cx: &mut Cx,
        value: &Expr,
        caps: &SurfaceCaps,
        states: &[A::State],
    ) -> Result<SplitRun> {
        let encoded = self.encode_once(codec, cx, value, caps)?;
        let frames = self.adapt_many(&encoded, states)?;
        Ok(SplitRun { encoded, frames })
    }
}

/// Result of a split run.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitRun {
    /// The single content-rate encoded Scene.
    pub encoded: EncodedScene,
    /// Device-rate local adaptation outputs.
    pub frames: Vec<Rc<Expr>>,
}

/// Adapts an already encoded Scene for many device states.
pub fn drive<A: LocalAdapter>(
    encoded: &EncodedScene,
    adapter: &A,
    states: &[A::State],
    profile: &DeviceProfile,
) -> Result<Vec<Rc<Expr>>> {
    states
        .iter()
        .map(|state| adapter.adapt(encoded, state, profile))
        .collect()
}
