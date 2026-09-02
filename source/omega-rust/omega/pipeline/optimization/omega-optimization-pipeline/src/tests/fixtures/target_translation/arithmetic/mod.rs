//! Optimizer module role: stage group. Exact binary-arithmetic Terminal fixtures.

mod exact_divide;
mod exact_multiply;
mod exact_remainder;
mod exact_subtract;
mod saturating_add_immediate;
mod saturating_subtract_immediate;
mod saturating_divide;
mod saturating_multiply;
mod saturating_remainder;
mod wrapping_add;
mod wrapping_add_immediate;
mod wrapping_divide;
mod wrapping_multiply;
mod wrapping_multiply_immediate;
mod wrapping_remainder;
mod wrapping_subtract;
mod wrapping_subtract_immediate;

pub(crate) use exact_divide::*;
pub(crate) use exact_multiply::*;
pub(crate) use exact_remainder::*;
pub(crate) use exact_subtract::*;
pub(crate) use saturating_add_immediate::*;
pub(crate) use saturating_subtract_immediate::*;
pub(crate) use saturating_divide::*;
pub(crate) use saturating_multiply::*;
pub(crate) use saturating_remainder::*;
pub(crate) use wrapping_add::*;
pub(crate) use wrapping_add_immediate::*;
pub(crate) use wrapping_divide::*;
pub(crate) use wrapping_multiply::*;
pub(crate) use wrapping_multiply_immediate::*;
pub(crate) use wrapping_remainder::*;
pub(crate) use wrapping_subtract::*;
pub(crate) use wrapping_subtract_immediate::*;
