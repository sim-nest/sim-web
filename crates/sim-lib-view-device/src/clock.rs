//! Deterministic frame clock for device-rate local adaptation.

use crate::RateClass;

/// Modeled device-rate clock used by [`crate::AdapterLoop`].
///
/// The clock is a caller-advanced tick index. It does not read wall time, sleep,
/// schedule work, or own an executor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameClock {
    /// Monotone device-frame tick.
    pub tick: u64,
    /// Timing envelope used to interpret tick distance.
    pub rate: RateClass,
}

impl FrameClock {
    /// Builds a clock at `tick` for the given rate envelope.
    pub fn new(tick: u64, rate: RateClass) -> Self {
        Self { tick, rate }
    }

    /// Builds a clock at tick zero.
    pub fn at_zero(rate: RateClass) -> Self {
        Self::new(0, rate)
    }

    /// Advances the modeled tick by one.
    pub fn advance(&mut self) {
        self.tick = self.tick.saturating_add(1);
    }

    /// Returns elapsed modeled milliseconds since `state_seq`.
    pub fn elapsed_ms_since(self, state_seq: u64) -> u64 {
        let elapsed_ticks = self.tick.saturating_sub(state_seq);
        elapsed_ticks.saturating_mul(1000) / u64::from(self.rate.adapt_hz.max(1))
    }

    /// Returns whether a sample from `state_seq` exceeds the stale window.
    pub fn stale(self, state_seq: u64) -> bool {
        self.elapsed_ms_since(state_seq) > u64::from(self.rate.max_stale_ms)
    }
}
