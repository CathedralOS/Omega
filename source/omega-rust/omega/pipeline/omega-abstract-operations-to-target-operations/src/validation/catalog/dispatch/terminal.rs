use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_scalar_crash, straight_line_unit_return,
};
use super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const UNIT_RETURN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineUnitReturn,
        straight_line_unit_return::is_candidate,
        straight_line_unit_return,
    );

pub(in crate::validation::catalog) const SCALAR_CRASH: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineScalarCrash,
        straight_line_scalar_crash::is_candidate,
        straight_line_scalar_crash,
    );

pub(super) fn straight_line_scalar_crash(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_scalar_crash::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineScalarCrash)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineScalarCrash)
}

pub(super) fn straight_line_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_unit_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineUnitReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineUnitReturn)
}
