//! Deterministic adapter loop with staleness and drop accounting.

use std::rc::Rc;

use sim_kernel::{Expr, Result};
use sim_value::build;

use crate::{DeviceProfile, EncodedScene, FrameClock, LocalAdapter};

/// Input consumed by one adapter-loop step.
#[derive(Clone, Debug, PartialEq)]
pub struct AdapterInput<S> {
    /// Latest content-rate encoded Scene.
    pub encoded: EncodedScene,
    /// Sequence of the encoded Scene.
    pub encoded_seq: u64,
    /// Freshest device-local state available for this step.
    pub state: S,
    /// Modeled tick sequence for `state`.
    pub state_seq: u64,
}

impl<S> AdapterInput<S> {
    /// Builds an input from an encoded Scene and state.
    pub fn new(encoded: EncodedScene, encoded_seq: u64, state: S, state_seq: u64) -> Self {
        Self {
            encoded,
            encoded_seq,
            state,
            state_seq,
        }
    }

    /// Builds an input from a shared encoded Scene pointer.
    pub fn from_shared_scene(scene: Rc<Expr>, encoded_seq: u64, state: S, state_seq: u64) -> Self {
        Self::new(
            EncodedScene::from_shared(scene),
            encoded_seq,
            state,
            state_seq,
        )
    }
}

/// Staleness behavior for adapter-loop steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StalePolicy {
    /// Reuse the last emitted frame, falling back to the encoded Scene.
    HoldLast,
    /// Run the adapter against the stale state and mark the frame stale.
    Predict,
    /// Emit a deterministic blank frame.
    Blank,
}

/// Output of one adapter-loop step.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Local adapter output.
    pub out: Rc<Expr>,
    /// Number of offered state updates coalesced before this step.
    pub dropped: u32,
    /// Whether the newest state exceeded the stale window.
    pub stale: bool,
    /// Modeled frame sequence.
    pub seq: u64,
    /// Sequence of the encoded Scene used for this step.
    pub encoded_seq: u64,
}

/// Device-rate loop over an already encoded Scene and a local adapter.
///
/// The loop is pure and deterministic. It has no runtime context, no
/// [`sim_lib_view::SurfaceCodec`], and no route back to content encoding.
#[derive(Clone, Debug)]
pub struct AdapterLoop<A> {
    adapter: A,
    policy: StalePolicy,
    last: Option<Rc<Expr>>,
    pending: u32,
}

impl<A> AdapterLoop<A> {
    /// Builds a loop for one adapter and staleness policy.
    pub fn new(adapter: A, policy: StalePolicy) -> Self {
        Self {
            adapter,
            policy,
            last: None,
            pending: 0,
        }
    }

    /// Returns the adapter.
    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    /// Returns the configured staleness policy.
    pub fn policy(&self) -> StalePolicy {
        self.policy
    }

    /// Returns the number of offered updates waiting for the next step.
    pub fn pending(&self) -> u32 {
        self.pending
    }
}

impl<A: LocalAdapter> AdapterLoop<A> {
    /// Records one available device-local state update.
    pub fn offer(&mut self, _state: &A::State) {
        self.pending = self.pending.saturating_add(1);
    }

    /// Advances the loop by one modeled frame.
    pub fn step(
        &mut self,
        clock: &FrameClock,
        input: &AdapterInput<A::State>,
        profile: &DeviceProfile,
    ) -> Result<Frame> {
        let dropped = self.pending.saturating_sub(1);
        self.pending = 0;
        let stale = clock.stale(input.state_seq);
        let out = if stale {
            match self.policy {
                StalePolicy::HoldLast => {
                    self.last.clone().unwrap_or_else(|| input.encoded.shared())
                }
                StalePolicy::Predict => {
                    self.adapter.adapt(&input.encoded, &input.state, profile)?
                }
                StalePolicy::Blank => Rc::new(blank_frame(profile)),
            }
        } else {
            self.adapter.adapt(&input.encoded, &input.state, profile)?
        };
        self.last = Some(Rc::clone(&out));
        Ok(Frame {
            out,
            dropped,
            stale,
            seq: clock.tick,
            encoded_seq: input.encoded_seq,
        })
    }
}

/// Builds the deterministic blank frame used by [`StalePolicy::Blank`].
pub fn blank_frame(profile: &DeviceProfile) -> Expr {
    build::map(vec![
        ("kind", build::qsym("device", "blank-frame")),
        ("tier", Expr::Symbol(profile.tier.to_symbol())),
    ])
}
