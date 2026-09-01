//! Optimizer module role: stage group. Exact-semantics and wrapping-arithmetic parameter receipt vocabulary.

mod exact_add;
mod saturating_add;
mod saturating_multiply;
mod saturating_subtract;
mod wrapping_add;
mod wrapping_multiply;
mod wrapping_subtract;

pub use exact_add::*;
pub use saturating_add::*;
pub use saturating_multiply::*;
pub use saturating_subtract::*;
pub use wrapping_add::*;
pub use wrapping_multiply::*;
pub use wrapping_subtract::*;
