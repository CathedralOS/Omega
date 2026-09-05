//! Exact catalog adapter for constant integer bitwise-XOR materialization.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;
use crate::validation::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_integer_bitwise_xor_immediate,
};

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseXorImmediate,
        straight_line_integer_bitwise_xor_immediate::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_bitwise_xor_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseXorImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseXorImmediate)
}
