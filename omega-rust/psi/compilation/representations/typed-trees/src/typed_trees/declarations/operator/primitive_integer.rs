//! Carrier roles after builtin integer operator selection.
//!
//! Source typing, captured proof arithmetic and executable construction share
//! this table. Selection and arithmetic policy remain their callers' duties;
//! a matching signature establishes neither representability nor authority.

use crate::expression::BinaryOperator;
use crate::types::PrimitiveType;

/// Result carrier for two already landed builtin integer operands.
pub fn builtin_integer_result_carrier(
    operator: BinaryOperator,
    [left, right]: [PrimitiveType; 2],
) -> Option<PrimitiveType> {
    use PrimitiveType::{Addr, U64};
    if !left.accepts_integer_literal() || !right.accepts_integer_literal() {
        return None;
    }
    if left == Addr || right == Addr {
        return match (operator, left, right) {
            (BinaryOperator::Add, Addr, U64)
            | (BinaryOperator::Add, U64, Addr)
            | (BinaryOperator::Subtract, Addr, U64) => Some(Addr),
            (BinaryOperator::Subtract, Addr, Addr) => Some(U64),
            _ => None,
        };
    }
    match operator {
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => Some(left),
        BinaryOperator::Add
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
            if left == right =>
        {
            Some(left)
        }
        _ => None,
    }
}

/// An anonymous peer of an address offset lands as a count. A left operand
/// of subtraction instead retains address context for an address distance.
/// Already typed operands never pass through this contextual landing rule.
pub fn integer_peer_operand_carrier(
    operator: BinaryOperator,
    peer: PrimitiveType,
    right_operand: bool,
) -> PrimitiveType {
    if peer == PrimitiveType::Addr
        && (operator == BinaryOperator::Add
            || (operator == BinaryOperator::Subtract && right_operand))
    {
        PrimitiveType::U64
    } else {
        peer
    }
}
