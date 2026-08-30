//! Optimizer module role: stage group. Exact parameter-arithmetic fixtures.

mod wrapping_integer_add;
mod wrapping_integer_multiply;
mod wrapping_integer_subtract;

pub(in crate::tests) use wrapping_integer_add::*;
pub(in crate::tests) use wrapping_integer_multiply::*;
pub(in crate::tests) use wrapping_integer_subtract::*;
