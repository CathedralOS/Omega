//! Proof-bearing divide and remainder catalog rows and typed adapters.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};
use super::super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const EXACT_INTEGER_DIVIDE: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineExactIntegerDivideParameters,
        straight_line_parameter::integer::arithmetic::exact_divide::is_candidate,
        exact_integer_divide,
    );

pub(in crate::validation::catalog) const EXACT_INTEGER_REMAINDER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineExactIntegerRemainderParameters,
        straight_line_parameter::integer::arithmetic::exact_remainder::is_candidate,
        exact_integer_remainder,
    );

pub(in crate::validation::catalog) const WRAPPING_INTEGER_DIVIDE: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerDivideParameters,
        straight_line_parameter::integer::arithmetic::wrapping_divide::is_candidate,
        wrapping_integer_divide,
    );

pub(in crate::validation::catalog) const WRAPPING_INTEGER_REMAINDER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerRemainderParameters,
        straight_line_parameter::integer::arithmetic::wrapping_remainder::is_candidate,
        wrapping_integer_remainder,
    );

pub(in crate::validation::catalog) const SATURATING_INTEGER_DIVIDE: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerDivideParameters,
        straight_line_parameter::integer::arithmetic::saturating_divide::is_candidate,
        saturating_integer_divide,
    );

pub(in crate::validation::catalog) const SATURATING_INTEGER_REMAINDER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerRemainderParameters,
        straight_line_parameter::integer::arithmetic::saturating_remainder::is_candidate,
        saturating_integer_remainder,
    );

pub(in crate::validation::catalog::dispatch) fn exact_integer_divide(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::exact_divide::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerDivideParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineExactIntegerDivideParameters)
}

pub(in crate::validation::catalog::dispatch) fn exact_integer_remainder(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::exact_remainder::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerRemainderParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineExactIntegerRemainderParameters)
}

pub(in crate::validation::catalog::dispatch) fn wrapping_integer_divide(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::wrapping_divide::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerDivideParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerDivideParameters)
}

pub(in crate::validation::catalog::dispatch) fn wrapping_integer_remainder(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::wrapping_remainder::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerRemainderParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerRemainderParameters)
}

pub(in crate::validation::catalog::dispatch) fn saturating_integer_divide(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::saturating_divide::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerDivideParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineSaturatingIntegerDivideParameters)
}

pub(in crate::validation::catalog::dispatch) fn saturating_integer_remainder(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::arithmetic::saturating_remainder::validate(
        source,
        expected_target,
        target,
    )
    .map(
        AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerRemainderParameters,
    )
    .map_err(
        AbstractToTargetTranslationFamilyError::StraightLineSaturatingIntegerRemainderParameters,
    )
}
