//! Package review records, bounded canonical encoding, and retained permission comparisons.
//! Checking and admitting a project require the compiler and Build issuers that own it.
#![forbid(unsafe_code)]

pub mod encoding;
pub mod permission_acceptance;
mod projection_error;
pub mod record;
pub use projection_error::{PackageReviewProjectionError, SemanticBindingProjectionError};
