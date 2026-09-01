//! Optimizer module role: executable entrance. Independently typed integer-shift source replay.

mod reconstruction;
pub(in crate::validation::straight_line_parameter) mod wrapping_left;
pub(in crate::validation::straight_line_parameter) mod wrapping_right;

pub(in crate::validation::straight_line_parameter) use reconstruction::{
    reconstruct_wrapping_left, reconstruct_wrapping_right,
};
