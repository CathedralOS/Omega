//! Optimizer module role: stage group. Terminal translation fixtures grouped by scalar semantic family.

use crate::tests::*;

mod arithmetic;
mod bitwise;
mod boolean_equal_immediate;
mod common;
mod comparison;
mod direct;
mod immediate;
mod integer_equal_immediate;
mod shift;
mod terminal;
mod unary;

pub(crate) use arithmetic::*;
pub(crate) use bitwise::*;
pub(crate) use boolean_equal_immediate::*;
pub(crate) use comparison::*;
pub(crate) use direct::*;
pub(crate) use immediate::*;
pub(crate) use integer_equal_immediate::*;
pub(crate) use shift::*;
pub(crate) use terminal::*;
pub(crate) use unary::*;
