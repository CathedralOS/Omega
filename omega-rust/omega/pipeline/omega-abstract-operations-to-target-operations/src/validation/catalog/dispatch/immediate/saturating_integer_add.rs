//! Exact catalog adapter for constant saturating integer-add materialization.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;
use crate::validation::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_saturating_integer_add_immediate,
};

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerAddImmediate,
        straight_line_saturating_integer_add_immediate::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_saturating_integer_add_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerAddImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineSaturatingIntegerAddImmediate)
}
