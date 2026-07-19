//! Timing envelopes for device surfaces.

use sim_kernel::{Expr, NumberLiteral};
use sim_value::{access, build};

/// The declared content, adaptation, and staleness timing envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RateClass {
    /// Content refresh cadence in hertz.
    pub content_hz: u16,
    /// Adapter loop cadence in hertz.
    pub adapt_hz: u16,
    /// Maximum stale data age accepted by the surface.
    pub max_stale_ms: u16,
}

impl RateClass {
    /// Safe fallback for missing rate metadata.
    pub fn safe_default() -> Self {
        Self {
            content_hz: 1,
            adapt_hz: 1,
            max_stale_ms: 1000,
        }
    }

    /// Low-rate watch-style envelope.
    pub fn watch() -> Self {
        Self::safe_default()
    }

    /// HUD-style envelope for low-bandwidth visual overlays.
    pub fn hud() -> Self {
        Self {
            content_hz: 5,
            adapt_hz: 30,
            max_stale_ms: 500,
        }
    }

    /// Stereo display envelope for rich pose-coupled surfaces.
    pub fn stereo() -> Self {
        Self {
            content_hz: 60,
            adapt_hz: 120,
            max_stale_ms: 100,
        }
    }

    /// Encodes this envelope as an open `rate` map.
    pub fn to_expr(self) -> Expr {
        build::map(vec![
            ("content-hz", build::uint(u64::from(self.content_hz))),
            ("adapt-hz", build::uint(u64::from(self.adapt_hz))),
            ("max-stale-ms", build::uint(u64::from(self.max_stale_ms))),
        ])
    }

    /// Parses a `rate` map.
    pub fn from_expr(expr: &Expr) -> Result<Self, RateError> {
        let Expr::Map(_) = expr else {
            return Err(RateError::NotRateMap);
        };
        Ok(Self {
            content_hz: rate_field(expr, "content-hz")?,
            adapt_hz: rate_field(expr, "adapt-hz")?,
            max_stale_ms: rate_field(expr, "max-stale-ms")?,
        })
    }

    /// Parses an optional `rate` map, using [`RateClass::safe_default`] when it
    /// is absent.
    pub fn from_optional_expr(expr: Option<&Expr>) -> Result<Self, RateError> {
        match expr {
            Some(expr) => Self::from_expr(expr),
            None => Ok(Self::safe_default()),
        }
    }
}

/// A reason a rate envelope could not be parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateError {
    /// The value was not a map.
    NotRateMap,
    /// A required rate field was missing.
    MissingField(&'static str),
    /// A rate field was not a non-negative integer literal.
    BadField(&'static str),
    /// A rate field was zero.
    ZeroField(&'static str),
    /// A rate field exceeded `u16`.
    OutOfRange(&'static str),
}

impl core::fmt::Display for RateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RateError::NotRateMap => write!(f, "rate value is not a map"),
            RateError::MissingField(name) => write!(f, "rate map missing field: {name}"),
            RateError::BadField(name) => write!(f, "rate field has wrong shape: {name}"),
            RateError::ZeroField(name) => write!(f, "rate field must be non-zero: {name}"),
            RateError::OutOfRange(name) => write!(f, "rate field exceeds u16: {name}"),
        }
    }
}

impl std::error::Error for RateError {}

fn rate_field(expr: &Expr, name: &'static str) -> Result<u16, RateError> {
    let Some(value) = access::field(expr, name) else {
        return Err(RateError::MissingField(name));
    };
    let Some(number) = integer_number(value) else {
        return Err(RateError::BadField(name));
    };
    let parsed = number
        .canonical
        .parse::<u64>()
        .map_err(|_| RateError::BadField(name))?;
    if parsed == 0 {
        return Err(RateError::ZeroField(name));
    }
    u16::try_from(parsed).map_err(|_| RateError::OutOfRange(name))
}

fn integer_number(expr: &Expr) -> Option<&NumberLiteral> {
    match expr {
        Expr::Number(number)
            if number.domain.namespace.is_none()
                && matches!(number.domain.name.as_ref(), "i64" | "u64") =>
        {
            Some(number)
        }
        _ => None,
    }
}
