//! Exact, saturating, and wrapping integer-arithmetic expression adapters.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};
use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const EXACT_INTEGER_ADD: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineExactIntegerAddParameters,
        straight_line_parameter::integer::arithmetic::exact_add::is_candidate,
        exact_integer_add,
    );

pub(in crate::validation::catalog) const WRAPPING_INTEGER_ADD: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerAddParameters,
        straight_line_parameter::integer::arithmetic::wrapping_add::is_candidate,
        wrapping_integer_add,
    );

pub(in crate::validation::catalog) const SATURATING_INTEGER_ADD: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerAddParameters,
        straight_line_parameter::integer::arithmetic::saturating_add::is_candidate,
        saturating_integer_add,
    );

pub(in crate::validation::catalog) const SATURATING_INTEGER_SUBTRACT: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerSubtractParameters,
        straight_line_parameter::integer::arithmetic::saturating_subtract::is_candidate,
        saturating_integer_subtract,
    );

pub(in crate::validation::catalog) const WRAPPING_INTEGER_SUBTRACT: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerSubtractParameters,
        straight_line_parameter::integer::arithmetic::wrapping_subtract::is_candidate,
        wrapping_integer_subtract,
    );

pub(in crate::validation::catalog) const WRAPPING_INTEGER_MULTIPLY: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerMultiplyParameters,
        straight_line_parameter::integer::arithmetic::wrapping_multiply::is_candidate,
        wrapping_integer_multiply,
    );

pub(in crate::validation::catalog::dispatch) fn wrapping_integer_add(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::wrapping_add::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerAddParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerAddParameters)
}

pub(in crate::validation::catalog::dispatch) fn saturating_integer_add(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::saturating_add::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerAddParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineSaturatingIntegerAddParameters)
}

pub(in crate::validation::catalog::dispatch) fn exact_integer_add(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::exact_add::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerAddParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineExactIntegerAddParameters)
}

pub(in crate::validation::catalog::dispatch) fn wrapping_integer_subtract(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::wrapping_subtract::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerSubtractParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerSubtractParameters)
}

pub(in crate::validation::catalog::dispatch) fn saturating_integer_subtract(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::saturating_subtract::validate(
        source,
        expected_target,
        target,
    )
    .map(
        AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerSubtractParameters,
    )
    .map_err(
        AbstractToTargetTranslationFamilyError::StraightLineSaturatingIntegerSubtractParameters,
    )
}

pub(in crate::validation::catalog::dispatch) fn wrapping_integer_multiply(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::wrapping_multiply::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerMultiplyParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerMultiplyParameters)
}
