//! Exact catalog adapter for constant Boolean equality materialization.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;
use crate::validation::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_boolean_equal_immediate,
};

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanEqualImmediate,
        straight_line_boolean_equal_immediate::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_boolean_equal_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanEqualImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineBooleanEqualImmediate)
}
