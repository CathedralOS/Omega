//! Optimizer module role: stage group. Exact wrapping-arithmetic parameter error vocabulary.

mod wrapping_add;
mod wrapping_multiply;
mod wrapping_subtract;

pub use wrapping_add::*;
pub use wrapping_multiply::*;
pub use wrapping_subtract::*;
