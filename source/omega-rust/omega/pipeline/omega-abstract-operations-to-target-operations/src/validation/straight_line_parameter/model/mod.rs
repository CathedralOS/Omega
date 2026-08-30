//! Optimizer module role: stage group. Immutable reconstruction vocabulary, partitioned by parameter expression shape.

mod bitwise;
mod comparison;
mod direct;
mod unary;

pub(super) use bitwise::*;
pub(super) use comparison::*;
pub(super) use direct::*;
pub(super) use unary::*;
