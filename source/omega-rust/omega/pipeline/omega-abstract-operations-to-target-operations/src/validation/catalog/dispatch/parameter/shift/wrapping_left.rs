//! Wrapping integer shift-left catalog row and typed adapter.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};
use super::super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const WRAPPING_INTEGER_SHIFT_LEFT: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerShiftLeftParameters,
        straight_line_parameter::integer::shift::wrapping_left::is_candidate,
        wrapping_integer_shift_left,
    );

fn wrapping_integer_shift_left(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::shift::wrapping_left::validate(
        source,
        expected_target,
        target,
    )
    .map(AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerShiftLeftParameters)
    .map_err(AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerShiftLeftParameters)
}
