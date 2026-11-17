use numerics::arithmetic::ArithmeticDomain;
use numerics::integer_carrier::FixedInteger;
use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
use tokens::{FloatLiteralKind, IntegerLiteralKind, NumericBase};

pub(crate) fn parse_integer_literal(
    text: &str,
    kind: IntegerLiteralKind,
) -> Result<IntegerLiteral, &'static str> {
    if kind.empty_digits {
        return Err("invalid integer literal");
    }

    let (radix, body) = match kind.base {
        NumericBase::Binary => (
            IntegerRadix::Binary,
            text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")),
        ),
        NumericBase::Octal => (
            IntegerRadix::Octal,
            text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")),
        ),
        NumericBase::Decimal => (IntegerRadix::Decimal, Some(text)),
        NumericBase::Hexadecimal => (
            IntegerRadix::Hexadecimal,
            text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")),
        ),
    };

    let body = body.ok_or("invalid integer literal")?;
    let (body, landed_type) = if kind.has_suffix {
        let (digits, carrier) = strip_integer_suffix(body)?;
        (digits, Some(carrier))
    } else {
        (body, None)
    };
    let literal = IntegerLiteral::from_parts(false, radix, body)?;
    Ok(match landed_type {
        Some(landed_type) => literal.with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
        None => literal,
    })
}

pub(crate) fn validate_float_literal(
    text: &str,
    kind: FloatLiteralKind,
) -> Result<(), &'static str> {
    if kind.empty_exponent {
        return Err("invalid float literal");
    }
    if text.ends_with("real") || text.ends_with("Real") {
        return Err(
            "the `real` literal suffix is retired; use an explicit f32/f64 format or a core Real embedding machine",
        );
    }
    if kind.has_suffix {
        strip_float_suffix(text)?;
    }

    Ok(())
}

fn strip_integer_suffix(text: &str) -> Result<(&str, LandedIntegerType), &'static str> {
    /*
        A carrier suffix is the carrier's own spelling: its sign
        letter, which no digit of any radix can be, then its width.
    */
    if let Some(start) = text.rfind(['i', 'u'])
        && let Some(carrier) = FixedInteger::from_decimal_spelling(&text[start..])
    {
        return Ok((&text[..start], LandedIntegerType::Fixed(carrier)));
    }

    Err("unknown integer literal suffix")
}

fn strip_float_suffix(text: &str) -> Result<&str, &'static str> {
    for suffix in FLOAT_SUFFIXES {
        if let Some(digits) = text.strip_suffix(suffix) {
            return Ok(digits);
        }
    }

    Err("unknown float literal suffix")
}

const FLOAT_SUFFIXES: &[&str] = &["f32", "f64"];

#[cfg(test)]
#[path = "../tests/unit/numbers.rs"]
mod tests;
