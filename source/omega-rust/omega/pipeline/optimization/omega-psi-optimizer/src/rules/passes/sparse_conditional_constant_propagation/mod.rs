//! Sparse conditional constant propagation rule-family entrance.
//!
//! `constant_evaluation` owns exact constant-fold proposals and their rule
//! contracts. `range_comparisons` owns comparisons decided from closed integer
//! ranges. The parent stage catalog selects this pass; this entrance owns its
//! exact local rule order.

mod constant_evaluation;
pub(in crate::rules::passes) mod range_comparisons;

pub use constant_evaluation::*;
pub use range_comparisons::{
    IntegerEqualConstantRangeRule, IntegerEqualRangeConstantRule, IntegerEqualRangeRangeRule,
    IntegerLessOrEqualConstantRangeRule, IntegerLessOrEqualRangeConstantRule,
    IntegerLessOrEqualRangeRangeRule, IntegerLessThanConstantRangeRule,
    IntegerLessThanRangeConstantRule, IntegerLessThanRangeRangeRule,
};

use crate::rules::catalog::BuiltInRuleRegistration;

/// The exact local rule order for this pass.
pub(in crate::rules) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, ExactIntegerAddConstantsRule),
        BuiltInRuleRegistration::new(1, ExactIntegerSubtractConstantsRule),
        BuiltInRuleRegistration::new(2, ExactIntegerMultiplyConstantsRule),
        BuiltInRuleRegistration::new(3, WrappingIntegerAddConstantsRule),
        BuiltInRuleRegistration::new(4, WrappingIntegerSubtractConstantsRule),
        BuiltInRuleRegistration::new(5, WrappingIntegerMultiplyConstantsRule),
        BuiltInRuleRegistration::new(6, SaturatingIntegerAddConstantsRule),
        BuiltInRuleRegistration::new(7, SaturatingIntegerSubtractConstantsRule),
        BuiltInRuleRegistration::new(8, SaturatingIntegerMultiplyConstantsRule),
        BuiltInRuleRegistration::new(9, ExactIntegerDivideConstantsRule),
        BuiltInRuleRegistration::new(10, ExactIntegerRemainderConstantsRule),
        BuiltInRuleRegistration::new(11, WrappingIntegerDivideConstantsRule),
        BuiltInRuleRegistration::new(12, WrappingIntegerRemainderConstantsRule),
        BuiltInRuleRegistration::new(13, SaturatingIntegerDivideConstantsRule),
        BuiltInRuleRegistration::new(14, SaturatingIntegerRemainderConstantsRule),
        BuiltInRuleRegistration::new(15, ExactIntegerShiftLeftConstantsRule),
        BuiltInRuleRegistration::new(16, ExactIntegerShiftRightConstantsRule),
        BuiltInRuleRegistration::new(17, WrappingIntegerShiftLeftConstantsRule),
        BuiltInRuleRegistration::new(18, WrappingIntegerShiftRightConstantsRule),
        BuiltInRuleRegistration::new(19, ExactIntegerCastConstantsRule),
        BuiltInRuleRegistration::new(20, IntegerWidenConstantsRule),
        BuiltInRuleRegistration::new(21, IntegerBitwiseNotConstantsRule),
        BuiltInRuleRegistration::new(22, IntegerBitwiseAndConstantsRule),
        BuiltInRuleRegistration::new(23, IntegerBitwiseOrConstantsRule),
        BuiltInRuleRegistration::new(24, IntegerBitwiseXorConstantsRule),
        BuiltInRuleRegistration::new(25, BooleanNotConstantsRule),
        BuiltInRuleRegistration::new(26, BooleanEqualConstantsRule),
        BuiltInRuleRegistration::new(27, IntegerEqualConstantsRule),
        BuiltInRuleRegistration::new(28, IntegerLessThanConstantsRule),
        BuiltInRuleRegistration::new(29, IntegerLessOrEqualConstantsRule),
        BuiltInRuleRegistration::new(30, IntegerLessThanRangeConstantRule),
        BuiltInRuleRegistration::new(31, IntegerLessThanConstantRangeRule),
        BuiltInRuleRegistration::new(32, IntegerLessOrEqualRangeConstantRule),
        BuiltInRuleRegistration::new(33, IntegerLessOrEqualConstantRangeRule),
        BuiltInRuleRegistration::new(34, IntegerEqualRangeConstantRule),
        BuiltInRuleRegistration::new(35, IntegerEqualConstantRangeRule),
        BuiltInRuleRegistration::new(36, IntegerEqualRangeRangeRule),
        BuiltInRuleRegistration::new(37, IntegerLessThanRangeRangeRule),
        BuiltInRuleRegistration::new(38, IntegerLessOrEqualRangeRangeRule),
    ]
}
