use psi_numerics::arithmetic::ArithmeticDomain;
use psi_numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
use psi_tokens::{FloatLiteralKind, IntegerLiteralKind, NumericBase};

/// Parse an integer literal token into its payload. UNSUFFIXED literals stay
/// ANONYMOUS (D14): the token is validated (digits legal for the radix) and
/// canonicalized, but NO numeric value is produced -- any magnitude is
/// representable, and the fit-check happens wherever a USE gives the literal
/// a type. A WIDTH SUFFIX (`0u32`) is the ch5 two-phase law's parse-site
/// landing (carrier CR4): the type is chosen AT the literal, so the landing
/// rides the payload from birth (Exact domain -- the decision-17 default; a
/// landed DESTINATION's domain still governs its folds, which prefer the
/// destination landing over an operand-derived one). `isize`/`usize`/`nat`
/// suffixes stay accepted-but-anonymous (no LandedIntegerType maps them;
/// `usize` is design-dead). Positions that genuinely need a number at parse
/// time go through `take_integer`'s i64 ceiling instead.
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
    let (body, landed_type) = if kind.has_suffix {
        strip_integer_suffix(body)?
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

pub(super) fn validate_float_literal(
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

fn strip_integer_suffix(text: &str) -> Result<(&str, Option<LandedIntegerType>), &'static str> {
    for (suffix, landed_type) in INTEGER_SUFFIXES {
        if let Some(digits) = text.strip_suffix(suffix) {
            return Ok((digits, *landed_type));
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

const INTEGER_SUFFIXES: &[(&str, Option<LandedIntegerType>)] = &[
    ("isize", None),
    ("usize", None),
    ("nat", None),
    ("Nat", None),
    ("i8", Some(LandedIntegerType::I8)),
    ("i16", Some(LandedIntegerType::I16)),
    ("i32", Some(LandedIntegerType::I32)),
    ("i64", Some(LandedIntegerType::I64)),
    ("u8", Some(LandedIntegerType::U8)),
    ("u16", Some(LandedIntegerType::U16)),
    ("u32", Some(LandedIntegerType::U32)),
    ("u64", Some(LandedIntegerType::U64)),
];

const FLOAT_SUFFIXES: &[&str] = &["f32", "f64"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_float_suffix_is_retired_with_a_directed_diagnostic() {
        let error = validate_float_literal(
            "3.0real",
            FloatLiteralKind {
                has_suffix: true,
                ..FloatLiteralKind::default()
            },
        )
        .expect_err("Real is an ordinary core carrier, not a literal landing");

        assert!(error.contains("`real` literal suffix is retired"));
    }
}
