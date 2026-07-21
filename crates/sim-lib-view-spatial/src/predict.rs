//! Prediction clamp for stale Viture pose reprojection.

use std::{cell::RefCell, rc::Rc};

use sim_kernel::{Expr, Result};
use sim_lib_view_device::{DeviceProfile, EncodedScene, LocalAdapter};

use crate::{PoseView, Reprojector};

/// Returns a pose whose prediction lead is bounded by `max_predict_ms`.
pub fn clamp_predicted(pose: &PoseView, max_predict_ms: u64) -> PoseView {
    let mut clamped = pose.clone();
    clamped.predict_ns = clamped
        .predict_ns
        .min(max_predict_ms.saturating_mul(1_000_000));
    clamped
}

/// Clamp-aware wrapper for Viture reprojection inside `StalePolicy::Predict`.
///
/// Fresh or merely stale poses reproject through the inner [`Reprojector`] with
/// bounded prediction. A pose whose own age exceeds the prediction clamp returns
/// the last good frame, so lost tracking cannot extrapolate into an unbounded
/// warp.
#[derive(Debug)]
pub struct ClampedReprojector {
    inner: Reprojector,
    last_good: RefCell<Option<Rc<Expr>>>,
}

impl ClampedReprojector {
    /// Builds a clamp-aware reprojector.
    pub fn new(max_predict_ms: u64) -> Self {
        Self {
            inner: Reprojector::new(max_predict_ms),
            last_good: RefCell::new(None),
        }
    }

    /// Returns the maximum prediction lead in milliseconds.
    pub fn max_predict_ms(&self) -> u64 {
        self.inner.max_predict_ms
    }

    /// Returns the inner reprojector.
    pub fn inner(&self) -> &Reprojector {
        &self.inner
    }
}

impl LocalAdapter for ClampedReprojector {
    type State = PoseView;

    fn adapt(
        &self,
        scene: &EncodedScene,
        state: &Self::State,
        profile: &DeviceProfile,
    ) -> Result<Rc<Expr>> {
        if pose_exceeds_clamp(state, self.max_predict_ms()) {
            return Ok(self
                .last_good
                .borrow()
                .clone()
                .unwrap_or_else(|| scene.shared()));
        }
        let clamped = clamp_predicted(state, self.max_predict_ms());
        let out = self.inner.adapt(scene, &clamped, profile)?;
        self.last_good.replace(Some(Rc::clone(&out)));
        Ok(out)
    }
}

pub(crate) fn pose_exceeds_clamp(pose: &PoseView, max_predict_ms: u64) -> bool {
    pose.age_ms > max_predict_ms
}
