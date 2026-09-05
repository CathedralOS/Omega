//! Exact catalog adapter for constant wrapping integer shift-left materialization.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;
use crate::validation::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_wrapping_integer_shift_left_immediate,
};

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerShiftLeftImmediate,
        straight_line_wrapping_integer_shift_left_immediate::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_wrapping_integer_shift_left_immediate::validate(source, target)
        .map(
            AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerShiftLeftImmediate,
        )
        .map_err(
            AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerShiftLeftImmediate,
        )
}
