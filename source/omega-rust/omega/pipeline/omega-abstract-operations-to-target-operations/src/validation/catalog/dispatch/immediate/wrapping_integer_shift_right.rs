//! Exact catalog adapter for constant wrapping integer shift-right materialization.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;
use crate::validation::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_wrapping_integer_shift_right_immediate,
};

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerShiftRightImmediate,
        straight_line_wrapping_integer_shift_right_immediate::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_wrapping_integer_shift_right_immediate::validate(source, target)
        .map(
            AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerShiftRightImmediate,
        )
        .map_err(
            AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerShiftRightImmediate,
        )
}
