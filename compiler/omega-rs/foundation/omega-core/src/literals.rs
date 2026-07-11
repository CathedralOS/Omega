//! Integer literals as UNINTERPRETED payloads (TASKS_TIME.md D14).
//!
//! A literal is a canonical SPELLING -- optional `-`, optional `0b`/`0o`/`0x`
//! prefix, then digits -- never a numeric value. Underscores, width suffixes,
//! uppercase prefixes/hex digits, and leading zeros are normalized away at
//! construction, so structural equality is spelling-insensitive WITHIN a radix.
//! (Cross-radix spellings of one value compare unequal at the node level; the
//! structural-equivalence proof paths treat that conservatively -- a loud
//! reject, never a soundness hole.)
//!
//! The only way to read a number out is an accessor that states the range it
//! accepts (`value_i64` today), so the fit-check happens at the USE. There is
//! deliberately NO unbounded numeric getter: the payload has no ceiling to
//! outgrow when wider integer types (u128, ...) arrive -- widening the language
//! means adding accessors, never touching stored literals. This mirrors how
//! float literals already ride the trees as uninterpreted `SourceText`.

use std::sync::{Arc, OnceLock};

/// Radix of an integer literal's digits. Deliberately local to omega-core so
/// the foundation layer does not depend on the token crate's `NumericBase`
/// (the parser maps between them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerRadix {
    Binary,
    Octal,
    Decimal,
    Hexadecimal,
}

impl IntegerRadix {
    fn base(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Binary => "0b",
            Self::Octal => "0o",
            Self::Decimal => "",
            Self::Hexadecimal => "0x",
        }
    }
}

/// An integer literal carried anonymously: canonical spelling, no value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerLiteral {
    /// Canonical form: `-`? (`0b`|`0o`|`0x`)? digits. Digits are lowercase,
    /// underscore-free, suffix-free, and carry no leading zeros (a lone `0`
    /// stays, and is never negative).
    text: Arc<str>,
}

impl IntegerLiteral {
    /// Build from parser-supplied parts. `digits` may contain underscores and
    /// uppercase hex digits; it must be non-empty after normalization and every
    /// character must be valid for `radix`. The width suffix must already be
    /// stripped (the parser owns suffix grammar).
    pub fn from_parts(
        negative: bool,
        radix: IntegerRadix,
        digits: &str,
    ) -> Result<Self, &'static str> {
        let mut normalized = String::with_capacity(digits.len());
        for character in digits.chars() {
            if character == '_' {
                continue;
            }
            if !character.is_digit(radix.base()) {
                return Err("invalid integer literal");
            }
            normalized.push(character.to_ascii_lowercase());
        }
        if normalized.is_empty() {
            return Err("invalid integer literal");
        }
        let trimmed = normalized.trim_start_matches('0');
        let digits = if trimmed.is_empty() { "0" } else { trimmed };
        let negative = negative && digits != "0";
        let sign = if negative { "-" } else { "" };
        Ok(Self {
            text: Arc::from(format!("{sign}{}{digits}", radix.prefix())),
        })
    }

    /// A literal the COMPILER synthesizes (desugarings, defaults, generated
    /// zeros). Canonical decimal spelling.
    pub fn from_value(value: i64) -> Self {
        if value == 0 {
            return Self::zero();
        }
        Self {
            text: Arc::from(value.to_string()),
        }
    }

    /// The shared zero literal (also the arena-default payload, mirroring the
    /// old `Integer(0)` default).
    pub fn zero() -> Self {
        static ZERO: OnceLock<Arc<str>> = OnceLock::new();
        Self {
            text: ZERO.get_or_init(|| Arc::from("0")).clone(),
        }
    }

    /// The literal with its sign flipped -- the parse-time negative fold
    /// (`-5` stays one constant), mirroring the Float fold that prepends `-`
    /// to the source text. `0` has no sign.
    pub fn negated(&self) -> Self {
        let text: &str = &self.text;
        if text == "0" {
            return self.clone();
        }
        let flipped = match text.strip_prefix('-') {
            Some(positive) => Arc::from(positive),
            None => Arc::from(format!("-{text}")),
        };
        Self { text: flipped }
    }

    /// The value, IF it fits i64 -- the transitional status-quo accessor.
    /// Every spelling the parser accepted before D14 fits by construction, so
    /// existing consumers behave bit-identically; only new u64-magnitude
    /// spellings return `None`, and a consumer must NEVER silently skip work
    /// on `None` (route to the oversize-literal validation gate instead).
    pub fn value_i64(&self) -> Option<i64> {
        let (sign, unsigned) = match self.text.strip_prefix('-') {
            Some(rest) => ("-", rest),
            None => ("", &*self.text),
        };
        let (base, digits) = split_radix(unsigned);
        if sign.is_empty() {
            i64::from_str_radix(digits, base).ok()
        } else {
            // Reattach the sign so `-9223372036854775808` (i64::MIN) parses.
            i64::from_str_radix(&format!("-{digits}"), base).ok()
        }
    }

    /// The EXACT value at any magnitude -- the proof engines' accessor
    /// (math roster N2: fact evaluation never rounds a literal). Canonical
    /// text always parses, so `None` is unreachable for well-formed
    /// literals; the Option mirrors the sibling accessors.
    pub fn value_bignum(&self) -> Option<crate::bignum::BigInt> {
        let (negative, unsigned) = match self.text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, &*self.text),
        };
        let (base, digits) = split_radix(unsigned);
        let magnitude = crate::bignum::BigInt::from_str_radix(digits, base)?;
        Some(if negative { magnitude.negate() } else { magnitude })
    }

    /// The value, IF it is non-negative and fits u64 — the u64-target window
    /// (D14 fire C). Widening the language to u128 later means adding another
    /// accessor here, never touching stored literals.
    pub fn value_u64(&self) -> Option<u64> {
        if self.text.starts_with('-') {
            return None;
        }
        let (base, digits) = split_radix(&self.text);
        u64::from_str_radix(digits, base).ok()
    }

    /// The literal's 8-byte two's-complement bit pattern: an i64-window value
    /// as its bits, or a u64-magnitude value verbatim. This is what an 8-byte
    /// store/immediate materializes; callers narrower than 8 bytes must go
    /// through the typed windows instead.
    pub fn bits_u64(&self) -> Option<u64> {
        self.value_i64()
            .map(|value| value as u64)
            .or_else(|| self.value_u64())
    }

    /// The canonical spelling (for display/diagnostics).
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl Default for IntegerLiteral {
    fn default() -> Self {
        Self::zero()
    }
}

impl std::fmt::Display for IntegerLiteral {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.text)
    }
}

fn split_radix(unsigned: &str) -> (u32, &str) {
    if let Some(digits) = unsigned.strip_prefix("0b") {
        (2, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0o") {
        (8, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0x") {
        (16, digits)
    } else {
        (10, unsigned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalization_is_spelling_insensitive_within_a_radix() {
        let plain = IntegerLiteral::from_parts(false, IntegerRadix::Hexadecimal, "FF").unwrap();
        let fancy = IntegerLiteral::from_parts(false, IntegerRadix::Hexadecimal, "0_0fF").unwrap();
        assert_eq!(plain, fancy);
        assert_eq!(plain.text(), "0xff");
    }

    #[test]
    fn value_i64_matches_the_old_parser_for_in_range_literals() {
        let literal = IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "42").unwrap();
        assert_eq!(literal.value_i64(), Some(42));
        assert_eq!(literal.negated().value_i64(), Some(-42));
        assert_eq!(IntegerLiteral::from_value(-7).value_i64(), Some(-7));
    }

    #[test]
    fn u64_magnitudes_parse_but_do_not_fit_i64() {
        let max = IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "18446744073709551615")
            .unwrap();
        assert_eq!(max.text(), "18446744073709551615");
        assert_eq!(max.value_i64(), None);
        assert_eq!(max.value_u64(), Some(u64::MAX));
        assert_eq!(max.bits_u64(), Some(u64::MAX));
    }

    #[test]
    fn u64_window_rejects_negatives_and_bits_are_twos_complement() {
        let negative = IntegerLiteral::from_value(-1);
        assert_eq!(negative.value_u64(), None);
        assert_eq!(negative.bits_u64(), Some(u64::MAX));
        let beyond =
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "18446744073709551616")
                .unwrap();
        assert_eq!(beyond.value_u64(), None);
        assert_eq!(beyond.bits_u64(), None);
    }

    #[test]
    fn i64_min_is_directly_spellable_via_negation() {
        let magnitude =
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "9223372036854775808")
                .unwrap();
        assert_eq!(magnitude.value_i64(), None);
        assert_eq!(magnitude.negated().value_i64(), Some(i64::MIN));
    }

    #[test]
    fn zero_never_carries_a_sign_and_double_negation_round_trips() {
        let zero = IntegerLiteral::from_parts(true, IntegerRadix::Decimal, "000").unwrap();
        assert_eq!(zero.text(), "0");
        assert_eq!(zero.negated().text(), "0");
        let five = IntegerLiteral::from_value(5);
        assert_eq!(five.negated().negated(), five);
    }

    #[test]
    fn invalid_digits_reject() {
        assert!(IntegerLiteral::from_parts(false, IntegerRadix::Binary, "102").is_err());
        assert!(IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "___").is_err());
    }
}
