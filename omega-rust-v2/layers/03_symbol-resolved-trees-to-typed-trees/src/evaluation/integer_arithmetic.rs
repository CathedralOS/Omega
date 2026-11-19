//! Integer primitive meanings shared by static index formation and prepared execution.
use crate::ValueType;
use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};

/// Maximum size of a mask or endpoint built during integer evaluation, in bits.
const INTEGER_ALLOCATION_BITS: u64 = 16 * 1024 * 1024;

pub(crate) fn compute(
    token: &str,
    operands: &[BigInt],
    carrier: &ValueType,
) -> Result<BigInt, String> {
    evaluate_integer_operation(token, operands, carrier).map_err(|error| match error {
        IntegerOperationError::OutsideCarrier(value) => format!("static index {value} exceeds {carrier}"),
        IntegerOperationError::QuotientOutsideCarrier => format!("static integer quotient exceeds {carrier}"),
        IntegerOperationError::ZeroDivisor => "integer division or remainder by zero".into(),
        IntegerOperationError::InvalidOperation => format!("unsupported static integer operation {token}"),
        IntegerOperationError::Refused(reason) => reason.into(),
    })
}

#[derive(Debug)]
pub enum IntegerOperationError {
    OutsideCarrier(BigInt),
    QuotientOutsideCarrier,
    ZeroDivisor,
    InvalidOperation,
    Refused(&'static str),
}

impl From<&'static str> for IntegerOperationError {
    fn from(reason: &'static str) -> Self {
        Self::Refused(reason)
    }
}

/// Apply a selected integer primitive to mathematical values. The caller owns
/// operand typing, proof obligations, and the diagnostic at the source occurrence.
pub fn evaluate_integer_operation(
    token: &str,
    operands: &[BigInt],
    carrier: &ValueType,
) -> Result<BigInt, IntegerOperationError> {
    let ValueType::Integer { signed, width } = carrier else {
        return Err("static operation requires an integer carrier".into());
    };
    let (op, policy) = if token.len() > 1 && token.ends_with(['%', '|', '!']) {
        token.split_at(token.len() - 1)
    } else {
        (token, "")
    };
    if !policy.is_empty() && width.is_none() {
        return Err("arithmetic policy requires a fixed-width carrier".into());
    }
    if operands.len() < 2 || (operands.len() != 2 && !matches!(op, "+" | "*")) || *width == Some(0) {
        return Err(IntegerOperationError::InvalidOperation);
    }
    let left = &operands[0];
    let right = &operands[1];
    let raw = match op {
        "+" => operands.iter().sum(),
        "*" => {
            let mut product = left.clone();
            for factor in &operands[1..] {
                if !product.is_zero() && !factor.is_zero() && product.bits().saturating_add(factor.bits()).saturating_sub(1) > INTEGER_ALLOCATION_BITS {
                    return Err("static evaluation exceeds the integer allocation budget".into());
                }
                product *= factor;
            }
            product
        }
        "-" => left - right,
        "/" | "%" if right.is_zero() => return Err(IntegerOperationError::ZeroDivisor),
        "/" | "%" => {
            let quotient = left / right;
            if op == "%" && !carrier.contains_integer(&quotient) && !matches!(policy, "%" | "|") {
                return Err(IntegerOperationError::QuotientOutsideCarrier);
            }
            if op == "/" { quotient } else { left % right }
        }
        "<<" | ">>" => {
            let width = width.ok_or("shift requires a fixed-width value")?;
            let count = if policy == "%" {
                let w = BigInt::from(width);
                ((right % &w + &w) % &w).to_u64().unwrap()
            } else {
                right
                    .to_u64()
                    .filter(|c| *c < width)
                    .ok_or("static shift count outside carrier width")?
            };
            if left.is_zero() {
                return Ok(BigInt::zero());
            }
            if op == "<<" && left.bits().saturating_add(count) > INTEGER_ALLOCATION_BITS {
                return Err("static evaluation exceeds the integer allocation budget".into());
            }
            if op == "<<" { left << count } else { left >> count }
        }
        "&" if width.is_some() && policy.is_empty() => left & right,
        "|" if width.is_some() && policy.is_empty() => left | right,
        "^" if width.is_some() && policy.is_empty() => left ^ right,
        _ => return Err(IntegerOperationError::InvalidOperation),
    };
    if carrier.contains_integer(&raw) {
        return Ok(raw);
    }
    if !matches!(policy, "%" | "|") {
        return Err(IntegerOperationError::OutsideCarrier(raw));
    }
    let width = width.ok_or("an arithmetic policy requires a fixed width")?;
    if !signed && policy == "|" && raw.sign() == num_bigint::Sign::Minus {
        return Ok(BigInt::zero());
    }
    if width > INTEGER_ALLOCATION_BITS {
        return Err("static evaluation exceeds the integer allocation budget".into());
    }
    let modulus = BigInt::one() << width;
    let lower = if *signed { -(&modulus >> 1u8) } else { BigInt::zero() };
    if policy == "%" {
        Ok(((&raw - &lower) % &modulus + &modulus) % &modulus + lower)
    } else {
        let upper = &lower + modulus - 1;
        Ok(raw.clamp(lower, upper))
    }
}
