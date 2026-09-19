//! Values used while discharging closed generic facts, not a proof IR.

use crate::preparation::generic_data::checked_fact_integer;
use crate::preparation::generic_data::const_integer_in_envelope;

use super::super::{BinaryOperator, Diagnostic, ExpressionHandle, SyntaxTrees};
use super::anonymous::AnonymousNumericValue;
use super::arguments::{ConstIntegerType, evaluate_declared_width_operation};
use std::cmp::Ordering;

pub(crate) enum ConstFactValue {
    Anonymous(AnonymousNumericValue),
    Integer(i128),
    DeclaredInteger {
        value: i128,
        integer_type: ConstIntegerType,
    },
    Boolean(bool),
}

/// The concrete scalar bound to `self` while a selected domain's facts replay
/// at declaration site, its closed index binders, and the payload a nested
/// membership carries into the next selected domain. Unlike `ConstFactValue`
/// there is no anonymous rational here: declaration-site discharge binds only
/// completed canonical values.
#[derive(Clone, Copy)]
pub(crate) enum ConstScalarValue {
    Integer(i128),
    DeclaredInteger {
        value: i128,
        integer_type: ConstIntegerType,
    },
    Boolean(bool),
}

impl ConstScalarValue {
    pub(crate) fn from_canonical(value: &super::super::CanonicalConstValue) -> Option<Self> {
        use language_semantics::const_value::DecodedCanonicalConstValue;
        match value.decode_encoding()? {
            DecodedCanonicalConstValue::Integer { value, type_name } => {
                let integer_type = ConstIntegerType::from_name(&type_name)?;
                integer_type.validate(value).ok()?;
                Some(Self::DeclaredInteger {
                    value,
                    integer_type,
                })
            }
            DecodedCanonicalConstValue::Boolean(value) => Some(Self::Boolean(value)),
            _ => None,
        }
    }

    pub(crate) fn into_fact_value(self) -> ConstFactValue {
        match self {
            Self::Integer(value) => ConstFactValue::Integer(value),
            Self::DeclaredInteger {
                value,
                integer_type,
            } => ConstFactValue::DeclaredInteger {
                value,
                integer_type,
            },
            Self::Boolean(value) => ConstFactValue::Boolean(value),
        }
    }
}

impl From<i128> for ConstScalarValue {
    fn from(value: i128) -> Self {
        Self::Integer(value)
    }
}

impl ConstFactValue {
    pub(crate) fn into_integer(
        self,
        syntax: &SyntaxTrees,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<i128>, String> {
        match self {
            Self::Anonymous(value) => value.into_integer(syntax, warnings).map(Some),
            Self::Integer(value) => Ok(Some(value)),
            Self::DeclaredInteger {
                value,
                integer_type,
            } => {
                integer_type.validate(value)?;
                Ok(Some(value))
            }
            Self::Boolean(_) => Ok(None),
        }
    }
}

pub(crate) fn evaluate_const_fact_binary(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    left: ConstFactValue,
    right: ConstFactValue,
    warnings: &mut Vec<Diagnostic>,
) -> Result<ConstFactValue, String> {
    use BinaryOperator::*;
    // A selected integer binding carries its width through every intermediate
    // result. A later comparison cannot hide an earlier Exact overflow, and an
    // untyped named value cannot borrow a peer's carrier as implicit authority.
    if matches!(&left, ConstFactValue::DeclaredInteger { .. })
        || matches!(&right, ConstFactValue::DeclaredInteger { .. })
    {
        return evaluate_declared_integer_fact(syntax, operator, left, right, warnings);
    }
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

fn evaluate_declared_integer_fact(
    syntax: &SyntaxTrees,
    operator: BinaryOperator,
    left: ConstFactValue,
    right: ConstFactValue,
    warnings: &mut Vec<Diagnostic>,
) -> Result<ConstFactValue, String> {
    use BinaryOperator::*;
    let shift = matches!(operator, ShiftLeft | ShiftRight);
    let integer_type = match (&left, &right) {
        (
            ConstFactValue::DeclaredInteger {
                integer_type: left, ..
            },
            ConstFactValue::DeclaredInteger {
                integer_type: right,
                ..
            },
        ) => {
            if !shift && left != right {
                return Err(format!(
                    "const fact operands have incompatible declared carriers `{}` and `{}`",
                    left.name(),
                    right.name()
                ));
            }
            *left
        }
        (ConstFactValue::DeclaredInteger { integer_type, .. }, ConstFactValue::Anonymous(_)) => {
            *integer_type
        }
        (ConstFactValue::Anonymous(_), ConstFactValue::DeclaredInteger { integer_type, .. })
            if !shift =>
        {
            *integer_type
        }
        _ => {
            return Err(
                "const fact integer operation requires retained declared carriers".to_owned(),
            );
        }
    };
    let mut operand = |value: ConstFactValue| -> Result<i128, String> {
        let (value, operand_type) = match value {
            ConstFactValue::DeclaredInteger {
                value,
                integer_type,
            } => (value, integer_type),
            ConstFactValue::Anonymous(value) => {
                (value.into_integer(syntax, warnings)?, integer_type)
            }
            _ => return Err("const fact integer operation lost its declared carrier".to_owned()),
        };
        // Shift counts have their own declared carrier. Their bounds and the
        // result width still come from the left payload, never from the count.
        operand_type.validate(value)?;
        Ok(value)
    };
    let left = operand(left)?;
    let right = operand(right)?;
    if right == 0 {
        match operator {
            Divide => return Err("division by zero is invalid".to_owned()),
            Modulo => return Err("remainder by zero is invalid".to_owned()),
            _ => {}
        }
    }
    let comparison = match operator {
        Equal => Some(left == right),
        NotEqual => Some(left != right),
        Greater => Some(left > right),
        GreaterOrEqual => Some(left >= right),
        Less => Some(left < right),
        LessOrEqual => Some(left <= right),
        _ => None,
    };
    if let Some(value) = comparison {
        return Ok(ConstFactValue::Boolean(value));
    }
    let value = match operator {
        Add => left.checked_add(right),
        Subtract => left.checked_sub(right),
        Multiply => left.checked_mul(right),
        Divide => left.checked_div(right),
        Modulo => {
            // Exact signed division must form even when only its remainder is
            // observed; MIN / -1 cannot be hidden by a zero remainder.
            let quotient = left
                .checked_div(right)
                .ok_or_else(|| "const fact division is undefined".to_owned())?;
            integer_type.validate(quotient)?;
            left.checked_rem(right)
        }
        ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => Some(
            evaluate_declared_width_operation(operator, left, right, Some(integer_type))?,
        ),
        _ => return Err("logical operators require boolean operands".to_owned()),
    }
    .ok_or_else(|| {
        "const fact integer operation is undefined or exceeds its declared range".to_owned()
    })?;
    integer_type.validate(value)?;
    Ok(ConstFactValue::DeclaredInteger {
        value,
        integer_type,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        BinaryOperator, ConstFactValue, ConstIntegerType, ExpressionHandle, SyntaxTrees,
        evaluate_const_fact_binary,
    };

    fn declared(value: i128, carrier: &str) -> ConstFactValue {
        ConstFactValue::DeclaredInteger {
            value,
            integer_type: ConstIntegerType::from_name(carrier).expect("integer carrier"),
        }
    }

    fn evaluate(
        operator: BinaryOperator,
        left: ConstFactValue,
        right: ConstFactValue,
    ) -> Result<ConstFactValue, String> {
        evaluate_const_fact_binary(
            &SyntaxTrees::default(),
            ExpressionHandle::invalid(),
            operator,
            left,
            right,
            &mut Vec::new(),
        )
    }

    #[test]
    fn declared_fact_intermediates_keep_width_before_comparison() {
        let result = evaluate(BinaryOperator::Add, declared(254, "u8"), declared(1, "u8"))
            .expect("representable sum");
        assert!(matches!(
            &result,
            ConstFactValue::DeclaredInteger { value: 255, integer_type }
                if integer_type.name() == "u8"
        ));
        assert!(matches!(
            evaluate(BinaryOperator::Greater, result, declared(254, "u8")),
            Ok(ConstFactValue::Boolean(true))
        ));
        assert!(evaluate(BinaryOperator::Add, declared(255, "u8"), declared(1, "u8")).is_err());
        assert!(
            evaluate(
                BinaryOperator::Subtract,
                declared(0, "u8"),
                declared(1, "u8")
            )
            .is_err()
        );
    }

    #[test]
    fn declared_fact_shifts_and_remainders_require_their_actual_width() {
        assert!(matches!(
            evaluate(
                BinaryOperator::ShiftRight,
                declared(-128, "i8"),
                declared(7, "i8")
            ),
            Ok(ConstFactValue::DeclaredInteger { value: -1, .. })
        ));
        assert!(
            evaluate(
                BinaryOperator::ShiftLeft,
                declared(1, "u8"),
                declared(8, "u8")
            )
            .is_err()
        );
        assert!(
            evaluate(
                BinaryOperator::Modulo,
                declared(-128, "i8"),
                declared(-1, "i8")
            )
            .is_err()
        );
        assert!(evaluate(BinaryOperator::Divide, declared(1, "u8"), declared(0, "u8")).is_err());
    }

    #[test]
    fn declared_fact_operands_cannot_borrow_another_bindings_carrier() {
        assert!(evaluate(BinaryOperator::Equal, declared(1, "u8"), declared(1, "u64")).is_err());
        assert!(
            evaluate(
                BinaryOperator::Add,
                declared(1, "u8"),
                ConstFactValue::Integer(1)
            )
            .is_err()
        );
    }
}
