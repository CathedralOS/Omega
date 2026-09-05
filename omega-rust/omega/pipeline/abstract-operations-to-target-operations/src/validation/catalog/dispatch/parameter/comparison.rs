//! Two-operand comparison-expression adapters.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};
use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const BOOLEAN_EQUAL: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanEqualParameters,
        straight_line_parameter::boolean::equal::is_candidate,
        boolean_equal,
    );

pub(in crate::validation::catalog) const INTEGER_EQUAL: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerEqualParameters,
        straight_line_parameter::integer::comparison::equal::is_candidate,
        integer_equal,
    );

pub(in crate::validation::catalog) const INTEGER_LESS_THAN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerLessThanParameters,
        straight_line_parameter::integer::comparison::less_than::is_candidate,
        integer_less_than,
    );

pub(in crate::validation::catalog) const INTEGER_LESS_OR_EQUAL: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualParameters,
        straight_line_parameter::integer::comparison::less_or_equal::is_candidate,
        integer_less_or_equal,
    );

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
