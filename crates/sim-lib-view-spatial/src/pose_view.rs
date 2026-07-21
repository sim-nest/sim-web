//! Device-local view state consumed by the stereo reprojector.

/// A pose-derived view sample for one device-rate reprojector step.
///
/// This type is intentionally not part of the content encoder. It is local
/// adapter state: the encoded `scene/spatial` packet can be reused while fresh
/// samples update the stereo eye views.
#[derive(Clone, Debug, PartialEq)]
pub struct PoseView {
    /// Monotone sample sequence number.
    pub sample_seq: u64,
    /// Age of the sample at the current adapter tick.
    pub age_ms: u64,
    /// Requested prediction lead in nanoseconds.
    pub predict_ns: u64,
    /// Head translation in meters relative to the encoded content origin.
    pub translation_m: [f64; 3],
    /// Yaw angle in degrees.
    pub yaw_deg: f64,
    /// Pitch angle in degrees.
    pub pitch_deg: f64,
    /// Roll angle in degrees.
    pub roll_deg: f64,
    /// Distance between eyes in meters.
    pub inter_eye_m: f64,
}

impl Default for PoseView {
    fn default() -> Self {
        Self::identity(0)
    }
}

impl PoseView {
    /// Builds an identity view sample for `sample_seq`.
    pub fn identity(sample_seq: u64) -> Self {
        Self {
            sample_seq,
            age_ms: 0,
            predict_ns: 0,
            translation_m: [0.0, 0.0, 0.0],
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            roll_deg: 0.0,
            inter_eye_m: 0.064,
        }
    }

    /// Returns this sample with a different age and prediction lead.
    pub fn with_timing(mut self, age_ms: u64, predict_ns: u64) -> Self {
        self.age_ms = age_ms;
        self.predict_ns = predict_ns;
        self
    }

    /// Returns this sample translated in meters.
    pub fn with_translation(mut self, translation_m: [f64; 3]) -> Self {
        self.translation_m = translation_m;
        self
    }

    /// Returns this sample with yaw, pitch, and roll in degrees.
    pub fn with_angles(mut self, yaw_deg: f64, pitch_deg: f64, roll_deg: f64) -> Self {
        self.yaw_deg = yaw_deg;
        self.pitch_deg = pitch_deg;
        self.roll_deg = roll_deg;
        self
    }

    /// Clamps the prediction lead to `max_predict_ms`.
    pub fn clamped_predict_ms(&self, max_predict_ms: u64) -> u64 {
        (self.predict_ns / 1_000_000).min(max_predict_ms)
    }

    pub(crate) fn clamped_yaw_rad(&self, max_predict_ms: u64) -> f64 {
        let requested_ms = self.predict_ns as f64 / 1_000_000.0;
        let scale = if requested_ms <= f64::EPSILON {
            0.0
        } else {
            self.clamped_predict_ms(max_predict_ms) as f64 / requested_ms
        };
        self.yaw_deg.to_radians() * scale
    }
}
