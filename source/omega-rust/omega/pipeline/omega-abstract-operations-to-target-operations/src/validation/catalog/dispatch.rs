use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_boolean_immediate, straight_line_integer_immediate, straight_line_parameter,
    straight_line_scalar_crash,
};
use super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(super) const STRAIGHT_LINE_INTEGER_IMMEDIATE: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
        straight_line_integer_immediate::is_candidate,
        straight_line_integer_immediate_adapter,
    );

pub(super) const STRAIGHT_LINE_BOOLEAN_IMMEDIATE: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanImmediate,
        straight_line_boolean_immediate::is_candidate,
        straight_line_boolean_immediate_adapter,
    );

pub(super) const STRAIGHT_LINE_SCALAR_CRASH: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineScalarCrash,
        straight_line_scalar_crash::is_candidate,
        straight_line_scalar_crash_adapter,
    );

pub(super) const STRAIGHT_LINE_INTEGER_PARAMETER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerParameter,
        straight_line_parameter::integer::is_candidate,
        straight_line_integer_parameter_adapter,
    );

pub(super) const STRAIGHT_LINE_BOOLEAN_PARAMETER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanParameter,
        straight_line_parameter::boolean::is_candidate,
        straight_line_boolean_parameter_adapter,
    );

fn straight_line_integer_immediate_adapter(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerImmediate)
}

fn straight_line_boolean_immediate_adapter(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_boolean_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineBooleanImmediate)
}

fn straight_line_scalar_crash_adapter(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_scalar_crash::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineScalarCrash)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineScalarCrash)
}

fn straight_line_integer_parameter_adapter(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::integer::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerParameter)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerParameter)
}

fn straight_line_boolean_parameter_adapter(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_parameter::boolean::validate(source, expected_target, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanParameter)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineBooleanParameter)
}
