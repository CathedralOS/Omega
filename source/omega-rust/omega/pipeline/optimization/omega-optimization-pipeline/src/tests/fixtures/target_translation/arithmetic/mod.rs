//! Optimizer module role: stage group. Exact binary-arithmetic Terminal fixtures.

mod saturating_multiply;
mod wrapping_add;
mod wrapping_multiply;
mod wrapping_subtract;

pub(crate) use saturating_multiply::*;
pub(crate) use wrapping_add::*;
pub(crate) use wrapping_multiply::*;
pub(crate) use wrapping_subtract::*;
