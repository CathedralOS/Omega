use omega_tokens::{FloatLiteralKind, IntegerLiteralKind, NumericBase};

pub(super) fn parse_integer_literal(
    text: &str,
    kind: IntegerLiteralKind,
) -> Result<i64, &'static str> {
    if kind.empty_digits {
        return Err("invalid integer literal");
    }

    let (radix, body) = match kind.base {
        NumericBase::Binary => (
            2,
            text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")),
        ),
        NumericBase::Octal => (
            8,
            text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")),
        ),
        NumericBase::Decimal => (10, Some(text)),
        NumericBase::Hexadecimal => (
            16,
            text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")),
        ),
    };

    let body = body.ok_or("invalid integer literal")?;
    let body = if kind.has_suffix {
        strip_integer_suffix(body)?
    } else {
        body
    };
    let normalized: String = body.chars().filter(|character| *character != '_').collect();
    i64::from_str_radix(&normalized, radix).map_err(|_| {
        // Distinguish a too-LARGE (but otherwise well-formed) literal from genuinely
        // invalid digits. The literal value is carried as i64 through the IR, so a u64
        // literal above i64::MAX (a full-width mask, a u64::MAX sentinel) is well-formed
        // yet not representable; reporting it as "invalid" misleads. If it parses as i128
        // it is a magnitude overflow, not a syntax error -- name the real limitation.
        if i128::from_str_radix(&normalized, radix).is_ok() {
            "integer literal exceeds the i64 range (u64 literals above i64::MAX are not yet supported)"
        } else {
            "invalid integer literal"
        }
    })
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
