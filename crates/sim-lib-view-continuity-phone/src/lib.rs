//! Calm, replayable phone interaction for continuity sessions.
//!
//! The journal is the only durable interaction state. [`PhoneController`]
//! rebuilds its projection from accepted continuity turns before every render
//! and transition; the Scene and audio gate are disposable derivatives.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod phone;

pub use phone::*;
