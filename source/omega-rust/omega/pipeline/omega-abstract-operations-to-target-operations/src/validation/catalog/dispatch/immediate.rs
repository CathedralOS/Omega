use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_boolean_immediate, straight_line_integer_immediate,
};

pub(super) fn straight_line_integer_immediate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerImmediate)
}

pub(super) fn straight_line_boolean_immediate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_boolean_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineBooleanImmediate)
}
