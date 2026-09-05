#![forbid(unsafe_code)]

//! Checked-source product orchestration through Terminal Psi publication.
//!
//! This coordinator sequences lowering, selected optimization and publication.
//! Checked-source receipts stay beside the portable artifact, not inside it.

mod production;
pub use production::*;
