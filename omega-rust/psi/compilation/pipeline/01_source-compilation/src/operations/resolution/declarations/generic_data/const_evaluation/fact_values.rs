use super::BigInt;
/* Values used while discharging closed generic facts, not a proof IR. */

use super::super::{
    BinaryOperator, CanonicalConstValue, Diagnostic, ExpressionHandle, SyntaxTrees,
};
use super::anonymous::AnonymousNumericValue;
use super::arguments::validate_declared_integer;
use numeric_literals::LandedIntegerType;
use std::cmp::Ordering;

pub(crate) enum ConstFactValue {
    Anonymous(AnonymousNumericValue),
    Integer(BigInt),
    DeclaredInteger {
        value: BigInt,
        integer_type: LandedIntegerType,
    },
    Boolean(bool),
}

/*
    The concrete scalar bound to `self` while a selected domain's facts replay
    at declaration site, its closed index binders, and the payload a nested
    membership carries into the next selected domain. Unlike `ConstFactValue`
    there is no anonymous rational here: declaration-site discharge binds only
    completed canonical values.
*/
#[derive(Clone)]
pub(crate) enum ConstScalarValue {
    Integer(BigInt),
    DeclaredInteger {
        value: BigInt,
        integer_type: LandedIntegerType,
    },
    Boolean(bool),
}

impl ConstScalarValue {
    pub(crate) fn from_canonical(value: &super::super::CanonicalConstValue) -> Option<Self> {
        use application_identity::const_value::DecodedCanonicalConstValue;
        match value.decode_encoding()? {
            DecodedCanonicalConstValue::Integer {
                value,
                integer_type,
            } => {
                validate_declared_integer(integer_type, &value).ok()?;
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

impl From<BigInt> for ConstScalarValue {
    fn from(value: BigInt) -> Self {
        Self::Integer(value)
    }
}

impl From<i128> for ConstScalarValue {
    fn from(value: i128) -> Self {
        Self::Integer(BigInt::from(value))
    }
}

/*
    A bare generic-argument name that spells a const scalar value rather than
    naming a declaration: an already-canonical const atom, an integer literal,
    or a boolean literal. Recognition stops at the spelling. The canonical atom
    is handed back undecoded because each site keeps its own required-type
    check and decoding rule (a non-scalar or out-of-range atom must still be
    reported against the parameter type, not silently dropped).
*/
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConstScalarSpelling {
    Canonical(CanonicalConstValue),
    Integer(BigInt),
    Boolean(bool),
}

impl ConstScalarSpelling {
    pub(crate) fn from_bare_name(name: &str) -> Option<Self> {
        if let Some(value) = CanonicalConstValue::from_atom(name) {
            return Some(Self::Canonical(value));
        }
        if let Some(value) = BigInt::from_decimal_str(name) {
            return Some(Self::Integer(value));
        }
        application_identity::const_value::boolean_literal_spelling(name).map(Self::Boolean)
    }
}

impl ConstFactValue {
    pub(crate) fn into_integer(
        self,
        syntax: &SyntaxTrees,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<BigInt>, String> {
        match self {
            Self::Anonymous(value) => value.into_integer(syntax, warnings).map(Some),
            Self::Integer(value) => Ok(Some(value)),
            Self::DeclaredInteger {
                value,
                integer_type,
            } => {
                validate_declared_integer(integer_type, &value)?;
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
    let Some(left) = left.into_integer(syntax, warnings)? else {
        return Err("const fact operands have incompatible types".to_owned());
    };
    let Some(right) = right.into_integer(syntax, warnings)? else {
        return Err("const fact operands have incompatible types".to_owned());
    };
    match operator {
        Add | Subtract | Multiply | Divide | Modulo => {
            super::arguments::apply_integer_operator(operator, &left, &right, None)
                .map(ConstFactValue::Integer)
        }
        Equal => Ok(ConstFactValue::Boolean(left == right)),
        NotEqual => Ok(ConstFactValue::Boolean(left != right)),
        Greater => Ok(ConstFactValue::Boolean(left > right)),
        GreaterOrEqual => Ok(ConstFactValue::Boolean(left >= right)),
        Less => Ok(ConstFactValue::Boolean(left < right)),
        LessOrEqual => Ok(ConstFactValue::Boolean(left <= right)),
        And | Or => Err("logical operators require boolean operands".to_string()),
        ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => Err(
            "shifts and bitwise operators require retained declared-width semantics".to_string(),
        ),
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
        (
            ConstFactValue::DeclaredInteger { integer_type, .. },
            ConstFactValue::Anonymous(_) | ConstFactValue::Integer(_),
        ) => *integer_type,
        (
            ConstFactValue::Anonymous(_) | ConstFactValue::Integer(_),
            ConstFactValue::DeclaredInteger { integer_type, .. },
        ) if !shift => *integer_type,
        _ => {
            return Err(
                "const fact integer operation requires retained declared carriers".to_owned(),
            );
        }
    };
    let mut operand = |value: ConstFactValue| -> Result<BigInt, String> {
        let (value, operand_type) = match value {
            ConstFactValue::DeclaredInteger {
                value,
                integer_type,
            } => (value, integer_type),
            ConstFactValue::Anonymous(value) => {
                (value.into_integer(syntax, warnings)?, integer_type)
            }
            ConstFactValue::Integer(value) => (value, integer_type),
            _ => {
                return Err("const fact integer operation lost its declared carrier".to_owned());
            }
        };
        validate_declared_integer(operand_type, &value)?;
        Ok(value)
    };
    let left = operand(left)?;
    let right = operand(right)?;
    if right.is_zero() {
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
    let value =
        super::arguments::apply_integer_operator(operator, &left, &right, Some(integer_type))?;
    validate_declared_integer(integer_type, &value)?;
    Ok(ConstFactValue::DeclaredInteger {
        value,
        integer_type,
    })
}
