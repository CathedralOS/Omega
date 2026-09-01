//! Optimizer module role: stage group. Typed integer-shift translation failures.

mod wrapping_left;
mod wrapping_right;

pub use wrapping_left::*;
pub use wrapping_right::*;
