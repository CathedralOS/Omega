//! Facts about scalar expressions the lowerings share: parameter and local
//! positions, expression types, builtin operators and arithmetic domains.

use crate::values::scalar::expression_plans::ScalarLocal;
use checked_trees::{
    CheckedIntegerBinaryKind, CheckedOperatorFacts, CheckedOperatorResolutionStatus,
    CheckedScalarExpression,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

/// Whether an authored parameter occupies a position in the dense scalar
/// namespace that `CheckedScalarExpression::Parameter` indexes: a primitive
/// carrier that is not an `[erased]` binding occurrence. An erased primitive
/// parameter owns no position (contracts.md#explicit-erased-bindings), so the
/// retained parameters after it shift down, matching the stripped signature
/// plan in `execution::unit::calls::signatures`; a runtime read of it then
/// finds no position and fails closed here as well as in validation.
pub(crate) fn occupies_scalar_position(program: &TypedTrees, parameter: &StateParameter) -> bool {
    !parameter.relevance.is_erased()
        && program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
}

pub(crate) fn parameter_position(
    program: &TypedTrees,
    path: &typed_trees::expression::TableNamePath,
    parameters: &[StateParameter],
) -> Option<usize> {
    let members = program.expression_table.name_path_members(path.members);
    if path.symbol.is_valid() && path.head_symbol.is_valid() && path.symbol != path.head_symbol {
        return None;
    }
    (members.len() == 1)
        .then(|| {
            parameters.iter().position(|parameter| {
                if path.symbol.is_valid() {
                    parameter.symbol == path.symbol
                } else if path.head_symbol.is_valid() {
                    parameter.symbol == path.head_symbol
                } else {
                    parameter.name.as_str() == members[0].as_str()
                }
            })
        })
        .flatten()
}

pub(crate) fn local_position(
    program: &TypedTrees,
    expression: ExpressionHandle,
    path: &typed_trees::expression::TableNamePath,
    locals: &[ScalarLocal],
) -> Option<usize> {
    if path.symbol.is_valid() && path.head_symbol.is_valid() && path.symbol != path.head_symbol {
        return None;
    }
    (program
        .expression_table
        .name_path_members(path.members)
        .len()
        == 1)
        .then(|| {
            locals.iter().rposition(|local| {
                if local.is_mutable {
                    // Bare storage reads require both retained resolved identities.
                    // Spelling cannot repair a missing or stale storage handle.
                    local.symbol.is_valid()
                        && local.symbol == path.symbol
                        && local.symbol == path.head_symbol
                } else if path.symbol.is_valid() {
                    local.symbol == path.symbol
                } else if path.head_symbol.is_valid() {
                    local.symbol == path.head_symbol
                } else {
                    local.name == program.expression_table.display_name(expression)
                }
            })
        })
        .flatten()
}

pub(crate) fn scalar_expression_type(
    expression: &CheckedScalarExpression,
) -> Option<PrimitiveType> {
    expression.primitive_type()
}

pub(crate) fn is_integer(primitive: PrimitiveType) -> bool {
    !matches!(
        primitive,
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
    )
}

pub(crate) fn operator_is_builtin(
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
) -> bool {
    operators
        .expression_use(expression)
        .is_none_or(|operator_use| {
            operator_use.status == CheckedOperatorResolutionStatus::BuiltinFallback
        })
}

pub(crate) fn combine_arithmetic_domains(
    left: ArithmeticDomain,
    right: ArithmeticDomain,
) -> Option<ArithmeticDomain> {
    match (left, right) {
        (ArithmeticDomain::Exact, domain) | (domain, ArithmeticDomain::Exact) => Some(domain),
        (left, right) if left == right => Some(left),
        _ => None,
    }
}

pub(crate) fn checked_integer_binary_kind(
    operator: BinaryOperator,
    domain: ArithmeticDomain,
) -> Option<CheckedIntegerBinaryKind> {
    match (operator, domain) {
        (BinaryOperator::BitwiseAnd, _) => Some(CheckedIntegerBinaryKind::BitwiseAnd),
        (BinaryOperator::BitwiseOr, _) => Some(CheckedIntegerBinaryKind::BitwiseOr),
        (BinaryOperator::BitwiseXor, _) => Some(CheckedIntegerBinaryKind::BitwiseXor),
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingShiftLeft)
        }
        (BinaryOperator::ShiftRight, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingShiftRight)
        }
        (BinaryOperator::ShiftRight, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactShiftRight)
        }
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactShiftLeft)
        }
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Trapping) => {
            Some(CheckedIntegerBinaryKind::TrappingShiftLeft)
        }
        (BinaryOperator::ShiftRight, ArithmeticDomain::Trapping) => {
            Some(CheckedIntegerBinaryKind::TrappingShiftRight)
        }
        (BinaryOperator::Add, ArithmeticDomain::Exact) => Some(CheckedIntegerBinaryKind::ExactAdd),
        (BinaryOperator::Subtract, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactSubtract)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactMultiply)
        }
        (BinaryOperator::Divide, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactDivide)
        }
        (BinaryOperator::Modulo, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactRemainder)
        }
        (BinaryOperator::Divide, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingDivide)
        }
        (BinaryOperator::Modulo, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingRemainder)
        }
        (BinaryOperator::Divide, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingDivide)
        }
        (BinaryOperator::Modulo, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingRemainder)
        }
        (BinaryOperator::Add, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingAdd)
        }
        (BinaryOperator::Add, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingAdd)
        }
        (BinaryOperator::Subtract, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingSubtract)
        }
        (BinaryOperator::Subtract, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingSubtract)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingMultiply)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingMultiply)
        }
        _ => None,
    }
}
