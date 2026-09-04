use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
};

#[derive(Clone, Copy)]
pub(super) enum ConstProofValue {
    Integer(i64),
    Boolean(bool),
}

pub(super) fn evaluate_domain_fact_expression(
    typed: &TypedTrees,
    expression: ExpressionHandle,
    self_value: i64,
) -> Result<Option<ConstProofValue>, String> {
    match typed.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value
            .value_i64()
            .map(ConstProofValue::Integer)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "proof operand `{value}` does not fit the build-time evaluator's signed integer boundary"
                )
            }),
        ExpressionNode::Boolean(value) => Ok(Some(ConstProofValue::Boolean(*value))),
        ExpressionNode::Name(path) => {
            let [name] = typed.expression_table.name_path_members(path.members) else {
                return Ok(None);
            };
            Ok((name.as_str() == "self")
                .then_some(ConstProofValue::Integer(self_value)))
        }
        ExpressionNode::Binary(binary) => {
            let Some(left) = evaluate_domain_fact_expression(typed, binary.left, self_value)? else {
                return Ok(None);
            };
            let Some(right) = evaluate_domain_fact_expression(typed, binary.right, self_value)?
            else {
                return Ok(None);
            };
            evaluate_domain_fact_binary(binary.operator, left, right).map(Some)
        }
        ExpressionNode::Unary(unary) => {
            let Some(operand) = evaluate_domain_fact_expression(typed, unary.operand, self_value)?
            else {
                return Ok(None);
            };
            match (unary.operator, operand) {
                (UnaryOperator::BitwiseNot, ConstProofValue::Integer(value)) => {
                    Ok(Some(ConstProofValue::Integer(!value)))
                }
                (UnaryOperator::BitwiseNot, ConstProofValue::Boolean(_)) => {
                    Err("bitwise complement requires an integer proof operand".to_string())
                }
                (UnaryOperator::LogicalNot, ConstProofValue::Boolean(value)) => {
                    Ok(Some(ConstProofValue::Boolean(!value)))
                }
                (UnaryOperator::LogicalNot, ConstProofValue::Integer(_)) => {
                    Err("logical negation requires a boolean proof operand".to_string())
                }
            }
        }
        _ => Ok(None),
    }
}

fn evaluate_domain_fact_binary(
    operator: BinaryOperator,
    left: ConstProofValue,
    right: ConstProofValue,
) -> Result<ConstProofValue, String> {
    use BinaryOperator::*;
    match (left, right) {
        (ConstProofValue::Integer(left), ConstProofValue::Integer(right)) => match operator {
            Add => left
                .checked_add(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof addition overflows `i64`".to_string()),
            Subtract => left
                .checked_sub(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof subtraction overflows `i64`".to_string()),
            Multiply => left
                .checked_mul(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof multiplication overflows `i64`".to_string()),
            Divide => left
                .checked_div(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof division is invalid".to_string()),
            Modulo => left
                .checked_rem(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof remainder is invalid".to_string()),
            ShiftLeft => u32::try_from(right)
                .ok()
                .and_then(|amount| left.checked_shl(amount))
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof left shift exceeds the `i64` width".to_string()),
            ShiftRight => u32::try_from(right)
                .ok()
                .and_then(|amount| left.checked_shr(amount))
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof right shift exceeds the `i64` width".to_string()),
            BitwiseAnd => Ok(ConstProofValue::Integer(left & right)),
            BitwiseOr => Ok(ConstProofValue::Integer(left | right)),
            BitwiseXor => Ok(ConstProofValue::Integer(left ^ right)),
            Equal => Ok(ConstProofValue::Boolean(left == right)),
            NotEqual => Ok(ConstProofValue::Boolean(left != right)),
            Greater => Ok(ConstProofValue::Boolean(left > right)),
            GreaterOrEqual => Ok(ConstProofValue::Boolean(left >= right)),
            Less => Ok(ConstProofValue::Boolean(left < right)),
            LessOrEqual => Ok(ConstProofValue::Boolean(left <= right)),
            And | Or => Err("logical proof operators require boolean operands".to_string()),
        },
        (ConstProofValue::Boolean(left), ConstProofValue::Boolean(right)) => match operator {
            And => Ok(ConstProofValue::Boolean(left && right)),
            Or => Ok(ConstProofValue::Boolean(left || right)),
            Equal => Ok(ConstProofValue::Boolean(left == right)),
            NotEqual => Ok(ConstProofValue::Boolean(left != right)),
            _ => Err("arithmetic proof operators require integer operands".to_string()),
        },
        _ => Err("const proof operands have incompatible types".to_string()),
    }
}
