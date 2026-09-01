//! Optimizer module role: stage group. Exact binary-arithmetic Terminal fixtures.

mod exact_divide;
mod exact_multiply;
mod exact_remainder;
mod exact_subtract;
mod saturating_multiply;
mod wrapping_add;
mod wrapping_divide;
mod wrapping_multiply;
mod wrapping_subtract;

pub(crate) use exact_divide::*;
pub(crate) use exact_multiply::*;
pub(crate) use exact_remainder::*;
pub(crate) use exact_subtract::*;
pub(crate) use saturating_multiply::*;
pub(crate) use wrapping_add::*;
pub(crate) use wrapping_divide::*;
pub(crate) use wrapping_multiply::*;
pub(crate) use wrapping_subtract::*;
