//! Optimizer module role: stage group. Exact parameter-arithmetic fixtures.

mod exact_integer_add;
mod exact_integer_divide;
mod exact_integer_multiply;
mod exact_integer_remainder;
mod exact_integer_subtract;
mod saturating_integer_add;
mod saturating_integer_divide;
mod saturating_integer_multiply;
mod saturating_integer_remainder;
mod saturating_integer_subtract;
mod wrapping_integer_add;
mod wrapping_integer_divide;
mod wrapping_integer_multiply;
mod wrapping_integer_remainder;
mod wrapping_integer_subtract;

pub(in crate::tests) use exact_integer_add::*;
pub(in crate::tests) use exact_integer_divide::*;
pub(in crate::tests) use exact_integer_multiply::*;
pub(in crate::tests) use exact_integer_remainder::*;
pub(in crate::tests) use exact_integer_subtract::*;
pub(in crate::tests) use saturating_integer_add::*;
pub(in crate::tests) use saturating_integer_divide::*;
pub(in crate::tests) use saturating_integer_multiply::*;
pub(in crate::tests) use saturating_integer_remainder::*;
pub(in crate::tests) use saturating_integer_subtract::*;
pub(in crate::tests) use wrapping_integer_add::*;
pub(in crate::tests) use wrapping_integer_divide::*;
pub(in crate::tests) use wrapping_integer_multiply::*;
pub(in crate::tests) use wrapping_integer_remainder::*;
pub(in crate::tests) use wrapping_integer_subtract::*;
