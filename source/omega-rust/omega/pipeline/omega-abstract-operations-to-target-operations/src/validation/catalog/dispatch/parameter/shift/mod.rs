//! Optimizer module role: executable entrance. Independently typed integer-shift family rows.

mod wrapping_left;
mod wrapping_right;

pub(in crate::validation::catalog) use wrapping_left::WRAPPING_INTEGER_SHIFT_LEFT;
pub(in crate::validation::catalog) use wrapping_right::WRAPPING_INTEGER_SHIFT_RIGHT;
