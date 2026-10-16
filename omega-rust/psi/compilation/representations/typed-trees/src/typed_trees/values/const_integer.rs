//! Closed and open integer expressions used as const machine arguments.
//!
//! A const parameter is a value, so `Degree - 1` is an argument of that value,
//! not a type. The expression stays open while any operand is still a const
//! binder. After specialization replaces that binder with a decimal spelling,
//! the same expression closes to one integer and the call selects that
//! instance. Shifts stay out of this reader: their width is the binder's
//! carrier, which this classification does not invent.

use super::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable};
use symbols::SymbolHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstIntegerExpression {
    Closed(i128),
    Open,
}

/// Classify one const-argument expression.
///
/// `is_binder` reports a name that is still a const or value parameter.
/// Integer spellings with no symbol are closed. Anything else is not this
/// form of const argument.
pub fn classify(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    is_binder: &impl Fn(SymbolHandle, &str) -> bool,
) -> Option<ConstIntegerExpression> {
    match expressions.expression(expression) {
        ExpressionNode::Integer(literal) => literal
            .value_i64()
            .map(i128::from)
            .or_else(|| literal.value_u64().map(i128::from))
            .map(ConstIntegerExpression::Closed),
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            let [member] = members else {
                return None;
            };
            let spelling = member.as_str();
            if !path.symbol.is_valid()
                && let Ok(value) = spelling.parse::<i128>()
            {
                return Some(ConstIntegerExpression::Closed(value));
            }
            is_binder(path.symbol, spelling).then_some(ConstIntegerExpression::Open)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
            ) =>
        {
            let left = classify(expressions, binary.left, is_binder)?;
            let right = classify(expressions, binary.right, is_binder)?;
            match (left, right) {
                (ConstIntegerExpression::Closed(left), ConstIntegerExpression::Closed(right)) => {
                    apply(binary.operator, left, right).map(ConstIntegerExpression::Closed)
                }
                _ => Some(ConstIntegerExpression::Open),
            }
        }
        _ => None,
    }
}

fn apply(operator: BinaryOperator, left: i128, right: i128) -> Option<i128> {
    match operator {
        BinaryOperator::Add => left.checked_add(right),
        BinaryOperator::Subtract => left.checked_sub(right),
        BinaryOperator::Multiply => left.checked_mul(right),
        BinaryOperator::Divide => left.checked_div(right),
        BinaryOperator::Modulo => left.checked_rem(right),
        _ => None,
    }
}
