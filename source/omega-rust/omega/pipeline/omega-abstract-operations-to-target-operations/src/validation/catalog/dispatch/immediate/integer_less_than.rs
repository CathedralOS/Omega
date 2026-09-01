//! Exact catalog adapter for constant integer less-than materialization.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::model::TranslationFamilyDescriptor;
use crate::validation::{
    straight_line_integer_less_than_immediate, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamilyError,
};
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const DESCRIPTOR: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerLessThanImmediate,
        straight_line_integer_less_than_immediate::is_candidate,
        validate,
    );

fn validate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_less_than_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerLessThanImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerLessThanImmediate)
}
