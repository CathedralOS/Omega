//! Exact catalog adapter for proof-bearing saturating divide over constant operands.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::validation::{
    straight_line_saturating_integer_divide_immediate_operands,
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
};
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerDivideImmediateOperands,
        straight_line_saturating_integer_divide_immediate_operands::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_saturating_integer_divide_immediate_operands::validate(source, target)
        .map(
            AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerDivideImmediateOperands,
        )
        .map_err(
            AbstractToTargetTranslationFamilyError::StraightLineSaturatingIntegerDivideImmediateOperands,
        )
}
