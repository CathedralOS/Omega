/*
A fixed-width integer type is a sign and a positive width, nothing else.
`u32`, `i8` and every other built-in name are spellings of that pair
(`wiki/spec/language/numeric_values.md#width-indexed-integers`), so every
stage states an integer type as one `FixedInteger` and no stage keeps a list
of the widths it happens to know. A stage that can only serve some widths
asks for the width and refuses the others by name.

The width is numerical meaning, not storage: it may exceed any machine word
and any host integer. Range questions are therefore answered from bit lengths
(`admits`), and the host-integer endpoints (`bounds_i128`) exist only for
widths whose endpoints a host integer can hold.
*/

use crate::bignum::BigInt;
use std::fmt;
use std::num::NonZeroU64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum IntegerSign {
    Signed,
    Unsigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct FixedInteger {
    sign: IntegerSign,
    width: NonZeroU64,
}

impl FixedInteger {
    /* The byte carrier of text, byte views and byte arrays. */
    pub const U8: Self = Self::unsigned(8);
    /* The carrier of counts, lengths, indexes and widths. */
    pub const U64: Self = Self::unsigned(64);

    /* A zero width is not an integer type, so it has no carrier. */
    pub const fn new(sign: IntegerSign, width: u64) -> Option<Self> {
        match NonZeroU64::new(width) {
            Some(width) => Some(Self { sign, width }),
            None => None,
        }
    }

    /* For widths written in compiler source; a zero width fails the build. */
    pub const fn unsigned(width: u64) -> Self {
        match Self::new(IntegerSign::Unsigned, width) {
            Some(carrier) => carrier,
            None => panic!("an integer width is positive"),
        }
    }

    /* For widths written in compiler source; a zero width fails the build. */
    pub const fn signed(width: u64) -> Self {
        match Self::new(IntegerSign::Signed, width) {
            Some(carrier) => carrier,
            None => panic!("an integer width is positive"),
        }
    }

    pub const fn sign(self) -> IntegerSign {
        self.sign
    }

    pub const fn width(self) -> u64 {
        self.width.get()
    }

    pub const fn is_signed(self) -> bool {
        matches!(self.sign, IntegerSign::Signed)
    }

    /*
        Reads the decimal shorthand: `u` or `i`, then a positive width in
        canonical decimal (`u32`, `i8`). A leading zero, a sign, a zero width
        or a width beyond `u64` is not a spelling of any carrier, and bare
        `u`/`i` name the unbounded integers instead.
    */
    pub fn from_decimal_spelling(name: &str) -> Option<Self> {
        let (sign, digits) = match name.as_bytes().first()? {
            b'u' => (IntegerSign::Unsigned, &name[1..]),
            b'i' => (IntegerSign::Signed, &name[1..]),
            _ => return None,
        };
        let bytes = digits.as_bytes();
        if bytes.is_empty() || bytes[0] == b'0' || !bytes.iter().all(u8::is_ascii_digit) {
            return None;
        }
        Self::new(sign, digits.parse().ok()?)
    }

    /*
        The spellings the front end admits as type names today: the eight
        conventional widths. This is the one place that restriction lives;
        it goes when name resolution admits every width
        (`TASKS.md`, WIDTH-INDEXED-INTEGERS).
    */
    pub fn from_builtin_name(name: &str) -> Option<Self> {
        Self::from_decimal_spelling(name).filter(|carrier| carrier.native_scalar_bytes().is_some())
    }

    /*
        Whether the carrier contains the mathematical integer, decided from
        bit lengths so an enormous width never builds its endpoints.
    */
    pub fn admits(self, value: &BigInt) -> bool {
        let width = self.width();
        let within = |bits: usize, limit: u64| u64::try_from(bits).is_ok_and(|bits| bits <= limit);
        match (self.sign, value.is_negative()) {
            (IntegerSign::Unsigned, true) => false,
            (IntegerSign::Unsigned, false) => within(value.bit_length(), width),
            (IntegerSign::Signed, false) => within(value.bit_length(), width - 1),
            /* `-2^(width-1)` is the one negative value with `width` magnitude bits. */
            (IntegerSign::Signed, true) => within(
                value.abs().sub(&BigInt::from_u64(1)).bit_length(),
                width - 1,
            ),
        }
    }

    /*
        The least and greatest values as host integers, or `None` when an
        endpoint does not fit one. A caller that gets `None` must refuse the
        width or answer through `admits`; it must not narrow the type.
    */
    pub fn bounds_i128(self) -> Option<(i128, i128)> {
        let width = u32::try_from(self.width()).ok()?;
        match self.sign {
            IntegerSign::Unsigned if width <= 127 => Some((0, i128::MAX >> (127 - width))),
            IntegerSign::Signed if width <= 128 => {
                let least = i128::MIN >> (128 - width);
                Some((least, -(least + 1)))
            }
            _ => None,
        }
    }

    /*
        Whole bytes of the native scalar that holds this width, for the
        widths the realization serves today: exactly 8, 16, 32 and 64 bits.
        Every other width has no scalar size yet and its user refuses it.
    */
    pub fn native_scalar_bytes(self) -> Option<usize> {
        match self.width() {
            8 => Some(1),
            16 => Some(2),
            32 => Some(4),
            64 => Some(8),
            _ => None,
        }
    }
}

impl fmt::Display for FixedInteger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = match self.sign {
            IntegerSign::Signed => 'i',
            IntegerSign::Unsigned => 'u',
        };
        write!(formatter, "{sign}{}", self.width)
    }
}

#[cfg(test)]
mod tests {
    use super::{FixedInteger, IntegerSign};
    use crate::bignum::BigInt;

    #[test]
    fn the_decimal_shorthand_is_a_sign_and_a_canonical_positive_width() {
        for (spelling, sign, width) in [
            ("u8", IntegerSign::Unsigned, 8),
            ("i64", IntegerSign::Signed, 64),
            ("u1", IntegerSign::Unsigned, 1),
            ("i257", IntegerSign::Signed, 257),
            ("u18446744073709551615", IntegerSign::Unsigned, u64::MAX),
        ] {
            let carrier = FixedInteger::from_decimal_spelling(spelling).unwrap();
            assert_eq!(carrier, FixedInteger::new(sign, width).unwrap());
            assert_eq!(carrier.to_string(), spelling);
        }
        for spelling in [
            "u",
            "i",
            "u0",
            "i0",
            "u08",
            "u+8",
            "u8 ",
            "U8",
            "u18446744073709551616",
            "f32",
            "bool",
            "",
        ] {
            assert_eq!(FixedInteger::from_decimal_spelling(spelling), None);
        }
        assert_eq!(FixedInteger::new(IntegerSign::Unsigned, 0), None);
        assert_eq!(
            FixedInteger::unsigned(32),
            FixedInteger::from_decimal_spelling("u32").unwrap()
        );
        assert_ne!(FixedInteger::unsigned(32), FixedInteger::signed(32));
    }

    #[test]
    fn membership_agrees_with_the_endpoints_at_every_small_width() {
        for width in 1..=12_u64 {
            for carrier in [FixedInteger::unsigned(width), FixedInteger::signed(width)] {
                let (least, greatest) = carrier.bounds_i128().unwrap();
                let expected = if carrier.is_signed() {
                    (-(1_i128 << (width - 1)), (1_i128 << (width - 1)) - 1)
                } else {
                    (0, (1_i128 << width) - 1)
                };
                assert_eq!((least, greatest), expected);
                for value in (least - 2)..=(greatest + 2) {
                    assert_eq!(
                        carrier.admits(&BigInt::from_i128(value)),
                        (least..=greatest).contains(&value),
                        "{carrier} {value}"
                    );
                }
            }
        }
    }

    #[test]
    fn wide_carriers_answer_membership_without_host_endpoints() {
        assert_eq!(
            FixedInteger::signed(128).bounds_i128(),
            Some((i128::MIN, i128::MAX))
        );
        assert_eq!(
            FixedInteger::unsigned(127).bounds_i128(),
            Some((0, i128::MAX))
        );
        assert_eq!(FixedInteger::unsigned(128).bounds_i128(), None);
        assert_eq!(FixedInteger::signed(129).bounds_i128(), None);

        let wide = FixedInteger::unsigned(257);
        let top = BigInt::from_u64(1).shl_bits(257);
        assert!(wide.admits(&top.sub(&BigInt::from_u64(1))));
        assert!(!wide.admits(&top));
        let signed = FixedInteger::signed(257);
        let half = BigInt::from_u64(1).shl_bits(256);
        assert!(signed.admits(&half.negate()));
        assert!(!signed.admits(&half));
        assert!(!signed.admits(&half.negate().sub(&BigInt::from_u64(1))));
        assert!(FixedInteger::unsigned(u64::MAX).admits(&top));
    }

    #[test]
    fn only_whole_power_of_two_byte_widths_have_a_native_scalar_today() {
        assert_eq!(FixedInteger::unsigned(8).native_scalar_bytes(), Some(1));
        assert_eq!(FixedInteger::signed(64).native_scalar_bytes(), Some(8));
        for width in [1, 13, 24, 65, 128, 257] {
            assert_eq!(FixedInteger::unsigned(width).native_scalar_bytes(), None);
        }
    }
}
