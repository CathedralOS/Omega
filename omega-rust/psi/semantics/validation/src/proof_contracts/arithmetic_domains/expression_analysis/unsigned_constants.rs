//! Integer literal widths and full-width unsigned constants.
//!
//! The i64-backed interval cannot represent every u64 value, so bitwise
//! operators recover exact unsigned operands here. A landed literal keeps its
//! own width; the destination never widens it.

use crate::proof_contracts::arithmetic_domains::integer_ranges::known_u64_value;
use crate::proof_contracts::arithmetic_domains::{
    ExpressionHandle, ExpressionNode, PrimitiveType, TypedTrees, ValueEnvironment, bitwise,
};

/// A landed literal retains its own width inside a larger expression; the
/// destination does not widen its arithmetic. A suffix contributes no policy.
pub(super) fn integer_literal_primitive(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    use numerics::literals::LandedIntegerType;

    let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) else {
        return None;
    };
    Some(match literal.landing()?.landed_type {
        LandedIntegerType::I8 => PrimitiveType::I8,
        LandedIntegerType::I16 => PrimitiveType::I16,
        LandedIntegerType::I32 => PrimitiveType::I32,
        LandedIntegerType::I64 => PrimitiveType::I64,
        LandedIntegerType::U8 => PrimitiveType::U8,
        LandedIntegerType::U16 => PrimitiveType::U16,
        LandedIntegerType::U32 => PrimitiveType::U32,
        LandedIntegerType::U64 => PrimitiveType::U64,
        LandedIntegerType::Addr => return None,
    })
}

/// Recover full-width unsigned constants which the i64-backed interval cannot
/// represent. This evaluates only bitwise trees, through the shared carrier
/// evaluator; it never reinterprets an unsigned literal as a signed value.
pub(super) fn bitwise_known_unsigned(
    program: &TypedTrees,
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
        ExpressionNode::Binary(binary) => bitwise::binary_unsigned_value(
            binary.operator,
            primitive,
            bitwise_known_unsigned(program, environment, primitive, binary.left, depth + 1)?,
            bitwise_known_unsigned(program, environment, primitive, binary.right, depth + 1)?,
        ),
        _ => None,
    }
}
