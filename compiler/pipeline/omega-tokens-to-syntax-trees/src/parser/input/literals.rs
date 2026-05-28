use omega_tokens::{FloatLiteralKind, IntegerLiteralKind, NumericBase};

pub(super) fn parse_integer_literal(
    text: &str,
    kind: IntegerLiteralKind,
) -> Result<i64, &'static str> {
    if kind.empty_digits {
        return Err("invalid integer literal");
    }
    if kind.has_suffix {
        return Err("integer literal suffixes are not supported yet");
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
    let normalized: String = body.chars().filter(|character| *character != '_').collect();
    i64::from_str_radix(&normalized, radix).map_err(|_| "invalid integer literal")
}

pub(super) fn validate_float_literal(kind: FloatLiteralKind) -> Result<(), &'static str> {
    if kind.empty_exponent {
        return Err("invalid float literal");
    }
    if kind.has_suffix {
        return Err("float literal suffixes are not supported yet");
    }

    Ok(())
}
