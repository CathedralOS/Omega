//! Optimizer module role: stage group. Exact binary-bitwise Terminal fixtures.

mod bitwise_and;
mod bitwise_and_immediate;
mod bitwise_or;
mod bitwise_or_immediate;
mod bitwise_xor;

pub(crate) use bitwise_and::*;
pub(crate) use bitwise_and_immediate::*;
pub(crate) use bitwise_or::*;
pub(crate) use bitwise_or_immediate::*;
pub(crate) use bitwise_xor::*;
