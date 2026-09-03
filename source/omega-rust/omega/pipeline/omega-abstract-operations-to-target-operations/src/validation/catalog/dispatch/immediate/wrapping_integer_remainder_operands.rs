//! Exact catalog adapter for proof-bearing wrapping remainder over constant operands.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;
use crate::validation::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_wrapping_integer_remainder_immediate_operands,
};

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerRemainderImmediateOperands,
        straight_line_wrapping_integer_remainder_immediate_operands::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_wrapping_integer_remainder_immediate_operands::validate(source, target)
        .map(
            AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerRemainderImmediateOperands,
        )
        .map_err(
            AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerRemainderImmediateOperands,
        )
}
