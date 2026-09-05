//! Exact integer shift-right catalog row and typed adapter.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};
use super::super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const EXACT_INTEGER_SHIFT_LEFT: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineExactIntegerShiftLeftParameters,
        straight_line_parameter::integer::shift::exact_left::is_candidate,
        exact_integer_shift_left,
    );

fn exact_integer_shift_left(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::shift::exact_left::validate(source, expected_target, target)
        .map(
            AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerShiftLeftParameters,
        )
        .map_err(
            AbstractToTargetTranslationFamilyError::StraightLineExactIntegerShiftLeftParameters,
        )
}
