use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

mod boolean_equal;
mod integer_bitwise_and;
mod integer_bitwise_or;
mod integer_bitwise_xor;
mod integer_equal;
mod integer_less_or_equal;
mod integer_less_than;
mod saturating_integer_add;
mod saturating_integer_multiply;
mod saturating_integer_subtract;
mod wrapping_integer_add;
mod wrapping_integer_divide_operands;
mod wrapping_integer_multiply;
mod wrapping_integer_shift_left;
mod wrapping_integer_shift_right;
mod wrapping_integer_subtract;

pub(in crate::validation::catalog) use boolean_equal::DESCRIPTOR as BOOLEAN_EQUAL;
pub(in crate::validation::catalog) use integer_bitwise_and::DESCRIPTOR as INTEGER_BITWISE_AND;
pub(in crate::validation::catalog) use integer_bitwise_or::DESCRIPTOR as INTEGER_BITWISE_OR;
pub(in crate::validation::catalog) use integer_bitwise_xor::DESCRIPTOR as INTEGER_BITWISE_XOR;
pub(in crate::validation::catalog) use integer_equal::DESCRIPTOR as INTEGER_EQUAL;
pub(in crate::validation::catalog) use integer_less_or_equal::DESCRIPTOR as INTEGER_LESS_OR_EQUAL;
pub(in crate::validation::catalog) use integer_less_than::DESCRIPTOR as INTEGER_LESS_THAN;
pub(in crate::validation::catalog) use saturating_integer_add::DESCRIPTOR as SATURATING_INTEGER_ADD;
pub(in crate::validation::catalog) use saturating_integer_multiply::DESCRIPTOR as SATURATING_INTEGER_MULTIPLY;
pub(in crate::validation::catalog) use saturating_integer_subtract::DESCRIPTOR as SATURATING_INTEGER_SUBTRACT;
pub(in crate::validation::catalog) use wrapping_integer_add::DESCRIPTOR as WRAPPING_INTEGER_ADD;
pub(in crate::validation::catalog) use wrapping_integer_divide_operands::DESCRIPTOR as WRAPPING_INTEGER_DIVIDE_OPERANDS;
pub(in crate::validation::catalog) use wrapping_integer_multiply::DESCRIPTOR as WRAPPING_INTEGER_MULTIPLY;
pub(in crate::validation::catalog) use wrapping_integer_shift_left::DESCRIPTOR as WRAPPING_INTEGER_SHIFT_LEFT;
pub(in crate::validation::catalog) use wrapping_integer_shift_right::DESCRIPTOR as WRAPPING_INTEGER_SHIFT_RIGHT;
pub(in crate::validation::catalog) use wrapping_integer_subtract::DESCRIPTOR as WRAPPING_INTEGER_SUBTRACT;

use super::super::super::{
    straight_line_boolean_immediate, straight_line_boolean_not_immediate,
    straight_line_integer_bitwise_not_immediate,
    straight_line_integer_exact_cast_immediate_operand, straight_line_integer_immediate,
    straight_line_integer_widen_immediate, AbstractToTargetFunctionTranslationReceipt,
    AbstractToTargetTranslationFamilyError,
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
