#![forbid(unsafe_code)]

//! Package declarations, immutable source resolution, closure review, and root
//! policy for Omega's registry-free package manager.
//!
//! Start with the responsibility modules below. The crate keeps its historical
//! flat exports for callers, while implementation ownership follows the module
//! tree instead of accumulating in this root.

pub mod declarations;
pub mod resolution;
pub mod review;
mod storage;

pub use declarations::*;
pub use resolution::*;
pub use review::*;
