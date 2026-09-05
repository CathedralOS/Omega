//! Binary bitwise-expression adapters.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};
use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const INTEGER_AND: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseAndParameters,
        straight_line_parameter::integer::bitwise::bitwise_and::is_candidate,
        integer_and,
    );

pub(in crate::validation::catalog) const INTEGER_OR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseOrParameters,
        straight_line_parameter::integer::bitwise::bitwise_or::is_candidate,
        integer_or,
    );

pub(in crate::validation::catalog) const INTEGER_XOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseXorParameters,
        straight_line_parameter::integer::bitwise::bitwise_xor::is_candidate,
        integer_xor,
    );

pub(in crate::validation::catalog::dispatch) fn integer_and(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::bitwise::bitwise_and::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseAndParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseAndParameters)
}

pub(in crate::validation::catalog::dispatch) fn integer_or(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::bitwise::bitwise_or::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseOrParameters)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseOrParameters)
}

pub(in crate::validation::catalog::dispatch) fn integer_xor(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::bitwise::bitwise_xor::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseXorParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseXorParameters)
}
