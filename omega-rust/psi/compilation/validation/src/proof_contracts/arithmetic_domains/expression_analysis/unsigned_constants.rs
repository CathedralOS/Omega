//! Integer literal widths and full-width unsigned constants.
//!
//! The i64-backed interval cannot represent every u64 value, so bitwise
//! operators recover exact unsigned operands here. A landed literal keeps its
//! own width; the destination never widens it.

use crate::proof_contracts::arithmetic_domains::integer_ranges::known_u64_value;
use crate::proof_contracts::arithmetic_domains::{
    ExpressionHandle, ExpressionNode, PrimitiveType, ValueEnvironment, bitwise,
};

/// A landed literal retains its own width inside a larger expression; the
/// destination does not widen its arithmetic. A suffix contributes no policy.
pub(super) fn integer_literal_primitive(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    let literal = program.expression_table.integer_node(expression)?;
    PrimitiveType::non_address_landed_integer(literal.landing()?.landed_type)
}

/// Recover full-width unsigned constants which the i64-backed interval cannot
/// represent. This evaluates only bitwise trees, through the shared carrier
/// evaluator; it never reinterprets an unsigned literal as a signed value.
pub(super) fn bitwise_known_unsigned(
    program: &dyn crate::CheckingInput,
    environment: &ValueEnvironment,
    primitive: PrimitiveType,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<u64> {
    if depth >= 64
        || integer_literal_primitive(program, expression)
            .is_some_and(|literal| literal != primitive)
    {
        return None;
    }
    if let Some(value) = known_u64_value(program, environment, expression) {
        return Some(value);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::BitwiseNot =>
        {
            bitwise::complement_unsigned_value(
                primitive,
                bitwise_known_unsigned(program, environment, primitive, unary.operand, depth + 1)?,
            )
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::BitwiseAnd
                    | typed_trees::expression::BinaryOperator::BitwiseOr
                    | typed_trees::expression::BinaryOperator::BitwiseXor
            ) =>
        {
            bitwise::binary_unsigned_value(
                binary.operator,
                primitive,
                bitwise_known_unsigned(program, environment, primitive, binary.left, depth + 1)?,
                bitwise_known_unsigned(program, environment, primitive, binary.right, depth + 1)?,
            )
        }
        _ => None,
    }
}
