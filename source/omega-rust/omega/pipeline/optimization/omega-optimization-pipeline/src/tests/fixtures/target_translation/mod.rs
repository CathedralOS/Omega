//! Optimizer module role: stage group. Terminal translation fixtures grouped by scalar semantic family.

use crate::tests::*;

mod bitwise;
mod common;
mod comparison;
mod direct;
mod immediate;
mod terminal;
mod unary;

pub(crate) use bitwise::*;
pub(crate) use comparison::*;
pub(crate) use direct::*;
pub(crate) use immediate::*;
pub(crate) use terminal::*;
pub(crate) use unary::*;
