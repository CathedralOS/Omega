//! Wrapping integer-arithmetic expression adapters.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};
use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const WRAPPING_INTEGER_ADD: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerAddParameters,
        straight_line_parameter::integer::arithmetic::wrapping_add::is_candidate,
        wrapping_integer_add,
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
