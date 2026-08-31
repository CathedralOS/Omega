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
pub(in crate::rules::passes) use model::{
    IntegerRangeComparisonKind, IntegerRangePairComparisonKind,
};

#[cfg(test)]
pub(in crate::rules::passes) use against_constant::evaluate as evaluate_integer_range_comparison;
#[cfg(test)]
pub(in crate::rules::passes) use against_range::evaluate as evaluate_integer_range_pair_comparison;
