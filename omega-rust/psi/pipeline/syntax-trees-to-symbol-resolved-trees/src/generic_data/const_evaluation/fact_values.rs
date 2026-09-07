//! Values used while discharging closed generic facts, not a proof IR.

use super::anonymous::AnonymousNumericValue;
use super::*;
use std::cmp::Ordering;

pub(in crate::generic_data) enum ConstFactValue {
    Anonymous(AnonymousNumericValue),
    Integer(i128),
    Boolean(bool),
}

impl ConstFactValue {
    pub(in crate::generic_data) fn into_integer(
        self,
        syntax: &SyntaxTrees,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<i128>, String> {
        match self {
            Self::Anonymous(value) => value.into_integer(syntax, warnings).map(Some),
            Self::Integer(value) => Ok(Some(value)),
            Self::Boolean(_) => Ok(None),
        }
    }
}

pub(in crate::generic_data) fn evaluate_const_fact_binary(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    left: ConstFactValue,
    right: ConstFactValue,
    warnings: &mut Vec<Diagnostic>,
) -> Result<ConstFactValue, String> {
    use BinaryOperator::*;
    let (left, right) = match (left, right) {
        (ConstFactValue::Anonymous(left), ConstFactValue::Anonymous(right)) => {
            if matches!(operator, Add | Subtract | Multiply | Divide) {
                return left
                    .binary(right, operator, expression)
                    .map(ConstFactValue::Anonymous);
            }
            let ordering = left.value.cmp_value(&right.value);
            let comparison = match operator {
                Equal => Some(ordering == Ordering::Equal),
                NotEqual => Some(ordering != Ordering::Equal),
                Greater => Some(ordering == Ordering::Greater),
                GreaterOrEqual => Some(ordering != Ordering::Less),
                Less => Some(ordering == Ordering::Less),
                LessOrEqual => Some(ordering != Ordering::Greater),
                _ => None,
            };
            if let Some(value) = comparison {
                return Ok(ConstFactValue::Boolean(value));
            }
            (
                ConstFactValue::Anonymous(left),
                ConstFactValue::Anonymous(right),
            )
        }
        operands => operands,
    };
    if let (ConstFactValue::Boolean(left), ConstFactValue::Boolean(right)) = (&left, &right) {
        return match operator {
            And => Ok(ConstFactValue::Boolean(*left && *right)),
            Or => Ok(ConstFactValue::Boolean(*left || *right)),
            Equal => Ok(ConstFactValue::Boolean(left == right)),
            NotEqual => Ok(ConstFactValue::Boolean(left != right)),
            _ => Err("arithmetic and ordering operators require integer operands".to_string()),
        };
    }
    // A typed integer peer establishes an integer landing, not rational
    // semantics for the peer. Existing bitwise/shift facts also need integers.
    let Some(left) = left.into_integer(syntax, warnings)? else {
        return Err("const fact operands have incompatible types".to_owned());
    };
    let Some(right) = right.into_integer(syntax, warnings)? else {
        return Err("const fact operands have incompatible types".to_owned());
    };
    match operator {
        Add => checked_fact_integer(left.checked_add(right), "addition"),
        Subtract => checked_fact_integer(left.checked_sub(right), "subtraction"),
        Multiply => checked_fact_integer(left.checked_mul(right), "multiplication"),
        Divide => left
            .checked_div(right)
            .map(ConstFactValue::Integer)
            .ok_or_else(|| "division by zero is invalid".to_string()),
        Modulo => left
            .checked_rem(right)
            .map(ConstFactValue::Integer)
            .ok_or_else(|| "remainder by zero is invalid".to_string()),
        ShiftLeft if left >= 0 => u32::try_from(right)
            .ok()
            .filter(|amount| *amount < u64::BITS)
            .and_then(|amount| left.checked_shl(amount))
            .and_then(const_integer_in_envelope)
            .map(ConstFactValue::Integer)
            .ok_or_else(|| "left shift exceeds the `u64` width".to_string()),
        ShiftRight if left >= 0 => u32::try_from(right)
            .ok()
            .filter(|amount| *amount < u64::BITS)
            .and_then(|amount| left.checked_shr(amount))
            .map(ConstFactValue::Integer)
            .ok_or_else(|| "right shift exceeds the `u64` width".to_string()),
        BitwiseAnd if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left & right)),
        BitwiseOr if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left | right)),
        BitwiseXor if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left ^ right)),
        Equal => Ok(ConstFactValue::Boolean(left == right)),
        NotEqual => Ok(ConstFactValue::Boolean(left != right)),
        Greater => Ok(ConstFactValue::Boolean(left > right)),
        GreaterOrEqual => Ok(ConstFactValue::Boolean(left >= right)),
        Less => Ok(ConstFactValue::Boolean(left < right)),
        LessOrEqual => Ok(ConstFactValue::Boolean(left <= right)),
        And | Or => Err("logical operators require boolean operands".to_string()),
        ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => {
            Err("signed shifts and bitwise operators require declared-width semantics".to_string())
        }
    }
}
