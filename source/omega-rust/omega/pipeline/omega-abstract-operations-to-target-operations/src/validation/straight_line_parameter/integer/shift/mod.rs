//! Optimizer module role: executable entrance. Independently typed integer-shift target replay.

mod reconstruction;
pub(crate) mod wrapping_left;
pub(crate) mod wrapping_right;

pub(super) use reconstruction::{reconstruct_wrapping_left, reconstruct_wrapping_right};
