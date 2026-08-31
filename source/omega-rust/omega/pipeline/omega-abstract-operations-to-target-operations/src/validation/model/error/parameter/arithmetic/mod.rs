//! Optimizer module role: stage group. Exact-semantics and wrapping-arithmetic parameter error vocabulary.

mod exact_add;
mod wrapping_add;
mod wrapping_multiply;
mod wrapping_subtract;

pub use exact_add::*;
pub use wrapping_add::*;
pub use wrapping_multiply::*;
pub use wrapping_subtract::*;
