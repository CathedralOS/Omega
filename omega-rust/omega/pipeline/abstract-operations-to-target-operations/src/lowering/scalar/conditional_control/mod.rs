//! Optimizer module role: stage group. Conditional control lowering by Boolean or integer result, with shared edge binding.

mod bindings;
mod boolean;
mod integer;

pub(super) use boolean::{lower_boolean_block, lower_boolean_conditional};
pub(super) use integer::lower_integer_conditional;
