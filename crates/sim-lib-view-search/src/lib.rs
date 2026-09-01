//! An offline, evidence-preserving projection of canonical search records.
//!
//! This crate owns presentation only. It accepts provider-neutral records from
//! `sim-lib-search-core` and immutable web records from `sim-lib-web-core`; it
//! has no HTTP, provider-codec, transport, credential, or orchestration
//! dependency. Remote links are emitted only as inert labels plus explicit
//! [`SearchAction::RequestOpen`] values for a policy host to decide.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod search;

pub use search::*;
