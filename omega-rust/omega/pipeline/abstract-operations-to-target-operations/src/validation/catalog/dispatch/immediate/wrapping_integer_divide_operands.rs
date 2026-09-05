//! Exact catalog adapter for proof-bearing wrapping divide over constant operands.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;
use crate::validation::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_wrapping_integer_divide_immediate_operands,
};

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerDivideImmediateOperands,
        straight_line_wrapping_integer_divide_immediate_operands::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_wrapping_integer_divide_immediate_operands::validate(source, target)
        .map(
            AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerDivideImmediateOperands,
        )
        .map_err(
            AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerDivideImmediateOperands,
        )
}
