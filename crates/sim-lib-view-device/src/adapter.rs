//! Device-local adapters for already encoded Scenes.

use std::rc::Rc;

use sim_kernel::{Expr, Result};

use crate::DeviceProfile;

/// A content-rate Scene shared across device-rate local adaptations.
///
/// The shared pointer is part of the contract: a display-only mirror path can
/// reuse the exact encoded tree for every device frame without deep-cloning the
/// Scene.
#[derive(Clone, Debug, PartialEq)]
pub struct EncodedScene {
    scene: Rc<Expr>,
}

impl EncodedScene {
    /// Wraps an owned encoded Scene in shared storage.
    pub fn new(scene: Expr) -> Self {
        Self {
            scene: Rc::new(scene),
        }
    }

    /// Wraps an already shared encoded Scene.
    pub fn from_shared(scene: Rc<Expr>) -> Self {
        Self { scene }
    }

    /// Borrows the encoded Scene expression.
    pub fn expr(&self) -> &Expr {
        &self.scene
    }

    /// Clones the shared pointer to the encoded Scene.
    pub fn shared(&self) -> Rc<Expr> {
        Rc::clone(&self.scene)
    }
}

/// The latency-critical, device-local last step of surface projection.
///
/// A local adapter is pure: it receives an already encoded Scene, the freshest
/// device-local state, and the typed device profile. It does not receive a
/// runtime context, cannot call SIM code, and cannot re-enter the content
/// encoder.
pub trait LocalAdapter {
    /// Device-local state consumed by the adapter.
    type State;

    /// Adapts an already encoded Scene for one device state sample.
    fn adapt(
        &self,
        scene: &EncodedScene,
        state: &Self::State,
        profile: &DeviceProfile,
    ) -> Result<Rc<Expr>>;
}

/// Display-only adapter that mirrors the encoded Scene unchanged.
///
/// This is the degenerate device case: the same shared Scene pointer is returned
/// for every frame, so the tree is not cloned per adaptation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MirrorAdapter;

impl LocalAdapter for MirrorAdapter {
    type State = ();

    fn adapt(
        &self,
        scene: &EncodedScene,
        _state: &Self::State,
        _profile: &DeviceProfile,
    ) -> Result<Rc<Expr>> {
        Ok(scene.shared())
    }
}
