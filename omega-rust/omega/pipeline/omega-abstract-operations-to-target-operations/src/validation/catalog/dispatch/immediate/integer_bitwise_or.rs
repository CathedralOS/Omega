//! Exact catalog adapter for constant integer bitwise-OR materialization.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;
use crate::validation::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_integer_bitwise_or_immediate,
};

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseOrImmediate,
        straight_line_integer_bitwise_or_immediate::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_bitwise_or_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseOrImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseOrImmediate)
}
