use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_scalar_crash,
};

pub(super) fn straight_line_scalar_crash(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_scalar_crash::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineScalarCrash)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineScalarCrash)
}
