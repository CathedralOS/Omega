//! Optimizer module role: stage group. Exact parameter-family receipt map.

mod arithmetic;
mod bitwise;
mod comparison;
mod direct;
mod shift;
mod unary;

pub use arithmetic::*;
pub use bitwise::*;
pub use comparison::*;
pub use direct::*;
pub use shift::*;
pub use unary::*;
