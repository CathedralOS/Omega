use crate::literals::numbers::{FloatLiteralKind, validate_float_literal};

/*
    A literal suffix is a carrier's own spelling, read as a sign
    and a width in every radix. A spelling that names no admitted
    carrier is refused instead of being read as digits.
*/
#[test]
fn an_integer_suffix_lands_the_literal_in_the_carrier_it_spells() {
    use crate::literals::numbers::{
        FixedInteger, IntegerLiteralKind, LandedIntegerType, NumericBase, parse_integer_literal,
    };
    let suffixed = |text: &str, base| {
        parse_integer_literal(
            text,
            IntegerLiteralKind {
                base,
                empty_digits: false,
                has_suffix: true,
            },
        )
    };
    for (text, base, value, carrier) in [
        ("7u8", NumericBase::Decimal, 7, FixedInteger::unsigned(8)),
        (
            "300i16",
            NumericBase::Decimal,
            300,
            FixedInteger::signed(16),
        ),
        (
            "0xffu32",
            NumericBase::Hexadecimal,
            255,
            FixedInteger::unsigned(32),
        ),
        ("0b101i64", NumericBase::Binary, 5, FixedInteger::signed(64)),
        /* Any width is a suffix; whether the value fits is typing's question. */
        ("7u13", NumericBase::Decimal, 7, FixedInteger::unsigned(13)),
        ("7i1", NumericBase::Decimal, 7, FixedInteger::signed(1)),
        (
            "0x1fu257",
            NumericBase::Hexadecimal,
            31,
            FixedInteger::unsigned(257),
        ),
    ] {
        let literal = suffixed(text, base).unwrap();
        assert_eq!(literal.value_i64(), Some(value), "{text}");
        assert_eq!(
            literal.landing().map(|landing| landing.landed_type),
            Some(LandedIntegerType::Fixed(carrier)),
            "{text}"
        );
    }
    /*
        `usize`, `isize` and the proof sorts are not carrier
        spellings, so their suffixes are refused rather than
        read as an unsuffixed literal.
    */
    for text in [
        "7u0", "7i0", "7u", "7i", "7q8", "7u08", "7usize", "7isize", "7nat", "7Nat",
    ] {
        assert_eq!(
            suffixed(text, NumericBase::Decimal).unwrap_err(),
            "unknown integer literal suffix",
            "{text}"
        );
    }
}

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
