use num_bigint::{BigInt, Sign};
use omega_typer::ValueType;

/// Bounds of the concrete carrier selected by typing. The evaluation fragment
/// caps widths so even a full product takes at most 256 magnitude bits.
pub(super) struct IntegerRange {
    minimum: BigInt,
    maximum: BigInt,
}

impl IntegerRange {
    pub(super) fn for_type(carrier: &ValueType) -> Option<Self> {
        let ValueType::Integer {
            signed,
            width: Some(width @ 1..=128),
        } = *carrier
        else {
            return None;
        };
        let limit = BigInt::from(1u8) << (width - u64::from(signed));
        let minimum = if signed { -&limit } else { BigInt::from(0u8) };
        Some(Self { minimum, maximum: limit - 1u8 })
    }

    pub(super) fn fits(
        &self,
        value: &BigInt,
    ) -> bool {
        self.minimum <= *value && *value <= self.maximum
    }

    pub(super) fn apply(
        &self,
        token: &str,
        left: BigInt,
        right: BigInt,
    ) -> Result<BigInt, &'static str> {
        let (result, overflow) = match token {
            "+" | "+%" | "+|" => (left + right, "exact addition exceeds its selected integer carrier"),
            "-" | "-%" | "-|" => (left - right, "exact subtraction exceeds its selected integer carrier"),
            "*" | "*%" | "*|" => (left * right, "exact multiplication exceeds its selected integer carrier"),
            _ => return Err("the selected body is outside the total integer fragment"),
        };
        if self.fits(&result) {
            return Ok(result);
        }
        if token.ends_with('|') {
            return Ok(result.clamp(self.minimum.clone(), self.maximum.clone()));
        }
        if token.ends_with('%') {
            let modulus = &self.maximum - &self.minimum + 1u8;
            let mut offset = (result - &self.minimum) % &modulus;
            if offset.sign() == Sign::Minus {
                offset += modulus;
            }
            return Ok(offset + &self.minimum);
        }
        Err(overflow)
    }
}
