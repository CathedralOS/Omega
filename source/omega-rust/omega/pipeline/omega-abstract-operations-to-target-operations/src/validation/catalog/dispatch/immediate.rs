use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

mod boolean_equal;
mod integer_equal;
mod integer_less_or_equal;
mod integer_less_than;

pub(in crate::validation::catalog) use boolean_equal::DESCRIPTOR as BOOLEAN_EQUAL;
pub(in crate::validation::catalog) use integer_equal::DESCRIPTOR as INTEGER_EQUAL;
pub(in crate::validation::catalog) use integer_less_or_equal::DESCRIPTOR as INTEGER_LESS_OR_EQUAL;
pub(in crate::validation::catalog) use integer_less_than::DESCRIPTOR as INTEGER_LESS_THAN;

use super::super::super::{
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError,
    straight_line_boolean_immediate, straight_line_boolean_not_immediate,
    straight_line_integer_bitwise_not_immediate,
    straight_line_integer_exact_cast_immediate_operand, straight_line_integer_immediate,
    straight_line_integer_widen_immediate,
};
use super::super::model::TranslationFamilyDescriptor;
use crate::AbstractToTargetTranslationFamily;

pub(in crate::validation::catalog) const INTEGER: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
        straight_line_integer_immediate::is_candidate,
        straight_line_integer_immediate,
    );

pub(in crate::validation::catalog) const BOOLEAN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanImmediate,
        straight_line_boolean_immediate::is_candidate,
        straight_line_boolean_immediate,
    );

pub(in crate::validation::catalog) const BOOLEAN_NOT: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineBooleanNotImmediate,
        straight_line_boolean_not_immediate::is_candidate,
        straight_line_boolean_not_immediate,
    );

pub(in crate::validation::catalog) const INTEGER_WIDEN: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerWidenImmediate,
        straight_line_integer_widen_immediate::is_candidate,
        straight_line_integer_widen_immediate,
    );

pub(in crate::validation::catalog) const INTEGER_BITWISE_NOT: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotImmediate,
        straight_line_integer_bitwise_not_immediate::is_candidate,
        straight_line_integer_bitwise_not_immediate,
    );

pub(in crate::validation::catalog) const INTEGER_EXACT_CAST_OPERAND: TranslationFamilyDescriptor =
    TranslationFamilyDescriptor::new(
        AbstractToTargetTranslationFamily::StraightLineIntegerExactCastImmediateOperand,
        straight_line_integer_exact_cast_immediate_operand::is_candidate,
        straight_line_integer_exact_cast_immediate_operand,
    );

pub(super) fn straight_line_integer_immediate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerImmediate)
}

pub(super) fn straight_line_boolean_immediate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_boolean_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineBooleanImmediate)
}

pub(super) fn straight_line_boolean_not_immediate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_boolean_not_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineBooleanNotImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineBooleanNotImmediate)
}

pub(super) fn straight_line_integer_widen_immediate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_widen_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerWidenImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerWidenImmediate)
}

pub(super) fn straight_line_integer_bitwise_not_immediate(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_bitwise_not_immediate::validate(source, target)
        .map(AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerBitwiseNotImmediate)
        .map_err(AbstractToTargetTranslationFamilyError::StraightLineIntegerBitwiseNotImmediate)
}

pub(super) fn straight_line_integer_exact_cast_immediate_operand(
    source: &AbstractFunction,
    _expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamilyError> {
    straight_line_integer_exact_cast_immediate_operand::validate(source, target)
        .map(
            AbstractToTargetFunctionTranslationReceipt::StraightLineIntegerExactCastImmediateOperand,
        )
        .map_err(
            AbstractToTargetTranslationFamilyError::StraightLineIntegerExactCastImmediateOperand,
        )
}
