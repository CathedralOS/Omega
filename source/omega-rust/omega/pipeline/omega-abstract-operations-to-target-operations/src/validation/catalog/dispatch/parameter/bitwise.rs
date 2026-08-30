//! Binary bitwise-expression adapters.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

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
