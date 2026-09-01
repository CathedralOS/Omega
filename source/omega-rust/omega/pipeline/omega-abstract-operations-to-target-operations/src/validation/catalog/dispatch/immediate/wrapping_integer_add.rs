//! Exact catalog adapter for constant integer wrapping integer addition materialization.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::validation::{
    straight_line_wrapping_integer_add_immediate, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamilyError,
};
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineWrappingIntegerAddImmediate,
        straight_line_wrapping_integer_add_immediate::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_wrapping_integer_add_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerAddImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineWrappingIntegerAddImmediate)
}
