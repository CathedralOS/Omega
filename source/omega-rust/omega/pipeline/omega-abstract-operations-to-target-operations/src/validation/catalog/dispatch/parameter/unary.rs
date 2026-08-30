//! Unary parameter-expression adapters.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};

pub(in crate::validation::catalog::dispatch) fn boolean_not(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::boolean::not::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanNotParameter)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineBooleanNotParameter)
}

pub(in crate::validation::catalog::dispatch) fn integer_bitwise_not(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::unary::bitwise_not::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotParameter)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseNotParameter)
}

pub(in crate::validation::catalog::dispatch) fn integer_widen(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::unary::widen::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerWidenParameter)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerWidenParameter)
}
