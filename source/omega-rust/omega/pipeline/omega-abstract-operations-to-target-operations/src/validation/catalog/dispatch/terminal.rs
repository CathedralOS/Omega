use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_port_write_unit_return, straight_line_scalar_crash,
    straight_line_trivial_affine_local_unit_return, straight_line_unit_call_return,
    straight_line_unit_return,
};
use super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const UNIT_RETURN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineUnitReturn,
        straight_line_unit_return::is_candidate,
        straight_line_unit_return,
    );

pub(in crate::validation::catalog) const PORT_WRITE_UNIT_RETURN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLinePortWriteUnitReturn,
        straight_line_port_write_unit_return::is_candidate,
        straight_line_port_write_unit_return,
    );

pub(in crate::validation::catalog) const UNIT_CALL_RETURN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineUnitCallReturn,
        straight_line_unit_call_return::is_candidate,
        straight_line_unit_call_return,
    );

pub(in crate::validation::catalog) const TRIVIAL_AFFINE_LOCAL_UNIT_RETURN:
    TranslationFamilyDescriptor = TranslationFamilyDescriptor::new(
    AbstractToTargetTranslationFamily::StraightLineTrivialAffineLocalUnitReturn,
    straight_line_trivial_affine_local_unit_return::is_candidate,
    straight_line_trivial_affine_local_unit_return,
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

pub(super) fn straight_line_port_write_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_port_write_unit_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLinePortWriteUnitReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLinePortWriteUnitReturn)
}

pub(super) fn straight_line_unit_call_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_unit_call_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineUnitCallReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineUnitCallReturn)
}

pub(super) fn straight_line_trivial_affine_local_unit_return(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_trivial_affine_local_unit_return::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineTrivialAffineLocalUnitReturn)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineTrivialAffineLocalUnitReturn)
}
