//! Optimizer module role: stage group. Immutable reconstruction vocabulary, partitioned by parameter expression shape.

mod arithmetic;
mod bitwise;
mod comparison;
mod direct;
mod shift;
mod unary;

pub(super) use arithmetic::*;
pub(super) use bitwise::*;
pub(super) use comparison::*;
pub(super) use direct::*;
pub(super) use shift::*;
pub(super) use unary::*;
