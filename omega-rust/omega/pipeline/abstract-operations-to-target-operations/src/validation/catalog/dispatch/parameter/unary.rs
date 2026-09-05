//! Unary parameter-expression adapters.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};
use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const BOOLEAN_NOT: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanNotParameter,
        straight_line_parameter::boolean::not::is_candidate,
        boolean_not,
    );

pub(in crate::validation::catalog) const INTEGER_BITWISE_NOT: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotParameter,
        straight_line_parameter::integer::unary::bitwise_not::is_candidate,
        integer_bitwise_not,
    );

pub(in crate::validation::catalog) const INTEGER_WIDEN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerWidenParameter,
        straight_line_parameter::integer::unary::widen::is_candidate,
        integer_widen,
    );

pub(in crate::validation::catalog) const INTEGER_EXACT_CAST: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerExactCastParameter,
        straight_line_parameter::integer::unary::exact_cast::is_candidate,
        integer_exact_cast,
    );

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

pub(in crate::validation::catalog::dispatch) fn integer_exact_cast(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::unary::exact_cast::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerExactCastParameter)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerExactCastParameter)
}
