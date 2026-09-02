//! Optimizer module role: stage group. Exact integer-shift Terminal fixtures.

mod exact_left;
mod exact_right;
mod wrapping_left;
mod wrapping_left_immediate;
mod wrapping_right;

pub(crate) use exact_left::*;
pub(crate) use exact_right::*;
pub(crate) use wrapping_left::*;
pub(crate) use wrapping_left_immediate::*;
pub(crate) use wrapping_right::*;
