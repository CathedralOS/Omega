//! Computational integer primitives used when forming a static index.
use crate::ValueType;
use num_bigint::{BigInt, Sign};
use num_traits::{One, ToPrimitive, Zero};

pub(crate) fn fits(
    value: &BigInt,
    carrier: &ValueType,
) -> bool {
    let ValueType::Integer { signed, width } = carrier else { return false };
    if !signed && value.sign() == Sign::Minus {
        return false;
    }
    width.is_none_or(|w| {
        if *signed && value.sign() == Sign::Minus {
            (value + 1u8).bits() < w
        } else if *signed {
            value.bits() < w
        } else {
            value.bits() <= w
        }
    })
}
pub(crate) fn compute(
    token: &str,
    operands: &[BigInt],
    carrier: &ValueType,
) -> Result<BigInt, String> {
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
    let left = &operands[0];
    let right = &operands[1];
    let raw = match op {
        "+" => operands.iter().sum(),
        "*" => operands.iter().product(),
        "-" => left - right,
        "/" | "%" if right.is_zero() => return Err("integer division or remainder by zero".into()),
        "/" | "%" => {
            let quotient = left / right;
            if op == "%" && !fits(&quotient, carrier) && !matches!(policy, "%" | "|") {
                return Err(format!("static integer quotient exceeds {carrier}"));
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
            if op == "<<" && count > 16 * 1024 * 1024 && !left.is_zero() {
                return Err("static evaluation exceeds the integer allocation budget".into());
            }
            if op == "<<" { left << count } else { left >> count }
        }
        "&" => left & right,
        "|" => left | right,
        "^" => left ^ right,
        _ => return Err(format!("unsupported static integer operation {token}")),
    };
    if fits(&raw, carrier) {
        return Ok(raw);
    }
    if !matches!(policy, "%" | "|") {
        return Err(format!("static index {raw} exceeds {carrier}"));
    }
    let width = width.ok_or("an arithmetic policy requires a fixed width")?;
    if width > 16 * 1024 * 1024 {
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
