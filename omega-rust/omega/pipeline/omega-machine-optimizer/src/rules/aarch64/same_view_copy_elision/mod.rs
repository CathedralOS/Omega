//! Optimizer module role: stage group. Shared AArch64 same-view-copy artifacts.
//!
//! Exact rule leaves own proposal and independent replay. This family owns
//! only their common authenticated disposition model, identity, and codec.

mod artifact;

#[cfg(test)]
pub(crate) mod test_support;

pub use artifact::*;
