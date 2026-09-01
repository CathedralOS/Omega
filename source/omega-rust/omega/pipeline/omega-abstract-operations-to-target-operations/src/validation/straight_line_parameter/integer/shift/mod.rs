//! Optimizer module role: executable entrance. Independently typed integer-shift target replay.

pub(crate) mod exact_left;
pub(crate) mod exact_right;
mod reconstruction;
pub(crate) mod wrapping_left;
pub(crate) mod wrapping_right;

pub(super) use reconstruction::{
    reconstruct_exact_left, reconstruct_exact_right, reconstruct_wrapping_left,
    reconstruct_wrapping_right,
};
