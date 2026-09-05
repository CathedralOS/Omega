//! Optimizer module role: stage group. Exact-semantics and wrapping-arithmetic parameter error vocabulary.

mod exact_add;
mod exact_divide;
mod exact_multiply;
mod exact_remainder;
mod exact_subtract;
mod saturating_add;
mod saturating_divide;
mod saturating_multiply;
mod saturating_remainder;
mod saturating_subtract;
mod wrapping_add;
mod wrapping_divide;
mod wrapping_multiply;
mod wrapping_remainder;
mod wrapping_subtract;

pub use exact_add::*;
pub use exact_divide::*;
pub use exact_multiply::*;
pub use exact_remainder::*;
pub use exact_subtract::*;
pub use saturating_add::*;
pub use saturating_divide::*;
pub use saturating_multiply::*;
pub use saturating_remainder::*;
pub use saturating_subtract::*;
pub use wrapping_add::*;
pub use wrapping_divide::*;
pub use wrapping_multiply::*;
pub use wrapping_remainder::*;
pub use wrapping_subtract::*;
