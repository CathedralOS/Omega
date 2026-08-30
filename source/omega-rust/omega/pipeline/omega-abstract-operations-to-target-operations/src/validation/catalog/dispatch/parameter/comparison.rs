//! Two-operand comparison-expression adapters.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};

pub(in crate::validation::catalog::dispatch) fn boolean_equal(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::boolean::equal::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanEqualParameters)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineBooleanEqualParameters)
}

pub(in crate::validation::catalog::dispatch) fn integer_equal(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::comparison::equal::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerEqualParameters)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerEqualParameters)
}

pub(in crate::validation::catalog::dispatch) fn integer_less_than(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::comparison::less_than::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLessThanParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerLessThanParameters)
}

pub(in crate::validation::catalog::dispatch) fn integer_less_or_equal(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::comparison::less_or_equal::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLessOrEqualParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerLessOrEqualParameters)
}
