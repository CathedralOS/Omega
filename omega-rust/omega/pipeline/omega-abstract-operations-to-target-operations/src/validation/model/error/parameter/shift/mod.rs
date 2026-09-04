//! Optimizer module role: stage group. Typed integer-shift translation failures.

mod exact_left;
mod exact_right;
mod wrapping_left;
mod wrapping_right;

pub use exact_left::*;
pub use exact_right::*;
pub use wrapping_left::*;
pub use wrapping_right::*;
