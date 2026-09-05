//! Wrapping integer shift-right catalog row and typed adapter.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};
use super::super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const WRAPPING_INTEGER_SHIFT_RIGHT: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerShiftRightParameters,
        straight_line_parameter::integer::shift::wrapping_right::is_candidate,
        wrapping_integer_shift_right,
    );

fn wrapping_integer_shift_right(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::shift::wrapping_right::validate(
        source,
        expected_target,
        target,
    )
    .map(
        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerShiftRightParameters,
    )
    .map_err(
        AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerShiftRightParameters,
    )
}
