//! Optimizer module role: stage group. Shared parameter reconstruction errors and exact family maps.

mod arithmetic;
mod bitwise;
mod comparison;
mod direct;
mod reconstruction;
mod shift;
mod unary;

pub use arithmetic::*;
pub use bitwise::*;
pub use comparison::*;
pub use direct::*;
pub(in crate::validation) use reconstruction::StraightLineParameterReconstructionError;
pub use shift::*;
pub use unary::*;
