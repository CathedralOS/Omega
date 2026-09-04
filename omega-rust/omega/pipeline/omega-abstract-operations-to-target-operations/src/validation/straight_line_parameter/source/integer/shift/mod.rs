//! Optimizer module role: executable entrance. Independently typed integer-shift source replay.

pub(in crate::validation::straight_line_parameter) mod exact_left;
pub(in crate::validation::straight_line_parameter) mod exact_right;
mod reconstruction;
pub(in crate::validation::straight_line_parameter) mod wrapping_left;
pub(in crate::validation::straight_line_parameter) mod wrapping_right;

pub(in crate::validation::straight_line_parameter) use reconstruction::{
    reconstruct_exact_left, reconstruct_exact_right, reconstruct_wrapping_left,
    reconstruct_wrapping_right,
};
