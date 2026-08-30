//! Optimizer module role: stage group. Exact parameter-family receipt map.

mod bitwise;
mod comparison;
mod direct;
mod unary;

pub use bitwise::*;
pub use comparison::*;
pub use direct::*;
pub use unary::*;
