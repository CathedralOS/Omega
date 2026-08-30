//! Direct parameter-return adapters.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_parameter,
};
use super::super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const INTEGER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerParameter,
        straight_line_parameter::integer::direct::is_candidate,
        integer,
    );

pub(in crate::validation::catalog) const BOOLEAN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanParameter,
        straight_line_parameter::boolean::direct::is_candidate,
        boolean,
    );

pub(in crate::validation::catalog::dispatch) fn integer(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::direct::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerParameter)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerParameter)
}

pub(in crate::validation::catalog::dispatch) fn boolean(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::boolean::direct::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanParameter)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineBooleanParameter)
}
