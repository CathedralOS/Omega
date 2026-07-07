use omega_core::literals::{IntegerLiteral, IntegerRadix};
use omega_tokens::{FloatLiteralKind, IntegerLiteralKind, NumericBase};

/// Parse an integer literal token into its ANONYMOUS payload (D14): the token
/// is validated (digits legal for the radix, known suffix) and canonicalized,
/// but NO numeric value is produced -- any magnitude is representable. The
/// fit-check happens wherever a USE gives the literal a type; positions that
/// genuinely need a number at parse time go through `take_integer`'s i64
/// ceiling instead.
pub(super) fn parse_integer_literal(
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
    let body = if kind.has_suffix {
        strip_integer_suffix(body)?
    } else {
        body
    };
    IntegerLiteral::from_parts(false, radix, body)
}

pub(super) fn validate_float_literal(
    text: &str,
    kind: FloatLiteralKind,
) -> Result<(), &'static str> {
    if kind.empty_exponent {
        return Err("invalid float literal");
    }
    if kind.has_suffix {
        strip_float_suffix(text)?;
    }

    Ok(())
}

fn strip_integer_suffix(text: &str) -> Result<&str, &'static str> {
    for suffix in INTEGER_SUFFIXES {
        if let Some(digits) = text.strip_suffix(suffix) {
            return Ok(digits);
        }
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

const INTEGER_SUFFIXES: &[&str] = &[
    "isize", "usize", "nat", "Nat", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64",
];

const FLOAT_SUFFIXES: &[&str] = &["real", "Real", "f32", "f64"];
