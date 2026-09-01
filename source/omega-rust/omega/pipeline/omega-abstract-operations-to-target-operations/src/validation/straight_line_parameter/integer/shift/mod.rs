//! Optimizer module role: executable entrance. Independently typed integer-shift target replay.

mod reconstruction;
pub(crate) mod wrapping_left;

pub(super) use reconstruction::reconstruct_wrapping_left;
