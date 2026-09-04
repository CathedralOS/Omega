//! Optimizer module role: stage group. Exact range-comparison rules by operand evidence shape.

mod against_constant;
mod against_range;
mod model;

pub use against_constant::{
    IntegerEqualConstantRangeRule, IntegerEqualRangeConstantRule,
    IntegerLessOrEqualConstantRangeRule, IntegerLessOrEqualRangeConstantRule,
    IntegerLessThanConstantRangeRule, IntegerLessThanRangeConstantRule,
};
pub use against_range::{
    IntegerEqualRangeRangeRule, IntegerLessOrEqualRangeRangeRule, IntegerLessThanRangeRangeRule,
};
#[cfg(test)]
pub(crate) use model::{IntegerRangeComparisonKind, IntegerRangePairComparisonKind};
