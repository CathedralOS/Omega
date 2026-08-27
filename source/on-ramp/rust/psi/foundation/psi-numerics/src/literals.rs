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

/// The integer type a constant LANDED at (ch5 "Constants: Two Phases" — the
/// two-phase law's phase-B fact, riding the literal payload so every clone,
/// splice, and table↔tree conversion carries it). Foundation-layer mirror of
/// the representations' integer `PrimitiveType` subset; the stamping sites map
/// between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandedIntegerType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    /// Pointer-width address (rides the 8-byte unsigned path).
    Addr,
}

impl LandedIntegerType {
    pub fn bit_width(self) -> u32 {
        match self {
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 => 32,
            Self::I64 | Self::U64 | Self::Addr => 64,
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32 | Self::I64)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Addr => "addr",
        }
    }
}

/// A landed constant's riding facts: the integer type it was rendered at and
/// the arithmetic domain governing further folds (decision 17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerLanding {
    pub landed_type: LandedIntegerType,
    pub domain: crate::arithmetic::ArithmeticDomain,
}

/// A float FORMAT a literal has landed at (F2, the float half of the ch5
/// two-phase law). Names mean formats permanently: `f32` never rebinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatFormat {
    F32,
    F64,
}

impl FloatFormat {
    pub fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

/// A float literal: the canonical SOURCE SPELLING plus an optional format
/// landing (F2, ch5 two-phase law). The spelling is the exact rational the
/// author wrote ("unitless until a site requests a type"); each format read
/// rounds ONCE, correctly, from the decimal spelling through the executable
/// FloatSemantics engine, so an f32 read NEVER routes through f64 (the
/// double-rounding residue this type retires). A width
/// suffix (`1.5f32`) is a parse-site landing, exactly the integer carrier's
/// CR4a. Equality is TEXT-ONLY like IntegerLiteral: spelling is identity,
/// the landing is metadata.
#[derive(Debug, Clone)]
pub struct FloatLiteral {
    text: Arc<str>,
    landing: Option<FloatFormat>,
}

impl PartialEq for FloatLiteral {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for FloatLiteral {}

impl Default for FloatLiteral {
    /// The arena-default payload (mirrors the old `bits: 0` = `0.0`).
    fn default() -> Self {
        Self::from_f64(0.0)
    }
}

impl FloatLiteral {
    /// Compat constructor for synthesized values (the old bits-based `new`).
    pub fn new(value: f64) -> Self {
        Self::from_f64(value)
    }

    /// Parse a source spelling (width/`real` suffixes stripped; a width
    /// suffix lands the format). `None` = not a float spelling.
    pub fn parse(source: &str) -> Option<Self> {
        let (body, landing) = strip_float_literal_suffix(source);
        // Validate the spelling once; the TEXT stays authoritative (reads
        // re-evaluate it exactly at their requested format).
        crate::bignum::ExactFloat::from_decimal_str(body)?;
        Some(Self {
            text: Arc::from(body),
            landing,
        })
    }

    /// A literal the COMPILER synthesizes. `{:?}` formatting is the shortest
    /// spelling that round-trips to the same f64 bits, so the text stays the
    /// exact value (specials included: `NaN`/`inf` re-parse).
    pub fn from_f64(value: f64) -> Self {
        Self {
            text: Arc::from(format!("{value:?}")),
            landing: None,
        }
    }

    /// The spelling, correctly rounded to f64.
    pub fn value_f64(&self) -> f64 {
        crate::float_semantics::FloatSemantics::from_decimal(
            crate::float_semantics::FloatFormat::BINARY64,
            &self.text,
        )
        .map(|meaning| meaning.to_f64())
        .unwrap_or(0.0)
    }

    /// The spelling, correctly rounded DIRECTLY to f32 (never via f64).
    pub fn value_f32(&self) -> f32 {
        crate::float_semantics::FloatSemantics::from_decimal(
            crate::float_semantics::FloatFormat::BINARY32,
            &self.text,
        )
        .map(|meaning| meaning.to_f32())
        .unwrap_or(0.0)
    }

    /// Transitional f64 window for pre-F2 consumers; landed reads go through
    /// `value_f32`/`value_f64` per the riding format.
    pub fn value(&self) -> f64 {
        self.value_f64()
    }

    /// The f32 bit pattern requested from this exact spelling. A format read
    /// always rounds directly from the rational source, even if a defensive
    /// backend path encounters an unstamped literal.
    pub fn f32_bits(&self) -> u32 {
        self.value_f32().to_bits()
    }

    /// The f64 read at the literal's landing: an F32-landed literal reads as
    /// its f32 value widened exactly (f32 -> f64 is lossless), so a suffixed
    /// literal means the same bits everywhere it flows.
    pub fn landed_f64(&self) -> f64 {
        match self.landing {
            Some(FloatFormat::F32) => f64::from(self.value_f32()),
            _ => self.value_f64(),
        }
    }

    pub fn with_landing(&self, landing: FloatFormat) -> Self {
        Self {
            text: self.text.clone(),
            landing: Some(landing),
        }
    }

    pub fn landing(&self) -> Option<FloatFormat> {
        self.landing
    }

    /// The parse-time negative fold (`-1.5` stays one constant); the landing
    /// rides, mirroring IntegerLiteral::negated.
    pub fn negated(&self) -> Self {
        let text: &str = &self.text;
        let flipped = match text.strip_prefix('-') {
            Some(positive) => Arc::from(positive),
            None => Arc::from(format!("-{text}")),
        };
        Self {
            text: flipped,
            landing: self.landing,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for FloatLiteral {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.text)
    }
}

fn strip_float_literal_suffix(source: &str) -> (&str, Option<FloatFormat>) {
    if let Some(body) = source.strip_suffix("f32") {
        return (body, Some(FloatFormat::F32));
    }
    if let Some(body) = source.strip_suffix("f64") {
        return (body, Some(FloatFormat::F64));
    }
    (source.trim_end_matches(['f', 'F']), None)
}

/// Radix of an integer literal's digits. Deliberately local to `psi-numerics` so
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

/// An integer literal: a canonical spelling (the exact mathematical value,
/// phase A of the two-phase law) plus, once landed, the landing riding along
/// (phase B). EQUALITY IS TEXT-ONLY by design: the spelling is the constant's
/// identity, the landing is metadata about how a SITE rendered it — two
/// spellings of one value stay equal whatever they landed as, preserving
/// every structural-equivalence consumer bit-for-bit. Consumers that need
/// landing-sensitivity compare `landing()` explicitly.
#[derive(Debug, Clone)]
pub struct IntegerLiteral {
    /// Canonical form: `-`? (`0b`|`0o`|`0x`)? digits. Digits are lowercase,
    /// underscore-free, suffix-free, and carry no leading zeros (a lone `0`
    /// stays, and is never negative).
    text: Arc<str>,
    /// `Some` once the constant has LANDED (ch5 two-phase law); `None` while
    /// anonymous. Rides every clone/splice/table↔tree conversion.
    landing: Option<IntegerLanding>,
}

impl PartialEq for IntegerLiteral {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for IntegerLiteral {}

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
            landing: None,
        })
    }

    /// A literal the COMPILER synthesizes (desugarings, defaults, generated
    /// zeros). Canonical decimal spelling, anonymous.
    pub fn from_value(value: i64) -> Self {
        if value == 0 {
            return Self::zero();
        }
        Self {
            text: Arc::from(value.to_string()),
            landing: None,
        }
    }

    /// The shared zero literal (also the arena-default payload, mirroring the
    /// old `Integer(0)` default).
    pub fn zero() -> Self {
        static ZERO: OnceLock<Arc<str>> = OnceLock::new();
        Self {
            text: ZERO.get_or_init(|| Arc::from("0")).clone(),
            landing: None,
        }
    }

    /// This literal LANDED at `landing` (ch5 two-phase law, phase B): the
    /// first typed site renders the value once and the fact rides from then
    /// on. Landing is idempotent-by-construction at the stamping sites; a
    /// re-stamp simply records the newest site's rendering.
    pub fn with_landing(&self, landing: IntegerLanding) -> Self {
        Self {
            text: self.text.clone(),
            landing: Some(landing),
        }
    }

    /// The landing this constant carries, if it has left the anonymous phase.
    pub fn landing(&self) -> Option<IntegerLanding> {
        self.landing
    }

    /// The literal with its sign flipped -- the parse-time negative fold
    /// (`-5` stays one constant), mirroring the Float fold that prepends `-`
    /// to the source text. `0` has no sign. The landing rides.
    pub fn negated(&self) -> Self {
        let text: &str = &self.text;
        if text == "0" {
            return self.clone();
        }
        let flipped = match text.strip_prefix('-') {
            Some(positive) => Arc::from(positive),
            None => Arc::from(format!("-{text}")),
        };
        Self {
            text: flipped,
            landing: self.landing,
        }
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
        Some(if negative {
            magnitude.negate()
        } else {
            magnitude
        })
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

    #[test]
    fn landing_rides_clones_and_negation_but_not_equality() {
        let landing = IntegerLanding {
            landed_type: LandedIntegerType::U32,
            domain: crate::arithmetic::ArithmeticDomain::Wrapping,
        };
        let anonymous = IntegerLiteral::from_value(5);
        let landed = anonymous.with_landing(landing);
        // Equality is TEXT-ONLY: spelling is identity, the landing is metadata.
        assert_eq!(anonymous, landed);
        assert_eq!(landed.landing(), Some(landing));
        assert_eq!(landed.clone().landing(), Some(landing));
        assert_eq!(landed.negated().landing(), Some(landing));
        assert_eq!(landed.negated().value_i64(), Some(-5));
        assert_eq!(anonymous.landing(), None);
        assert!(!LandedIntegerType::U32.is_signed());
        assert_eq!(LandedIntegerType::U32.bit_width(), 32);
        assert!(LandedIntegerType::I8.is_signed());
    }

    #[test]
    fn float_reads_round_directly_through_executable_semantics() {
        let witness = FloatLiteral::parse("8388609.499999999999999").unwrap();
        assert_eq!(witness.f32_bits(), 0x4b00_0001);

        let landed = FloatLiteral::parse("1.000000059604644775390625f32").unwrap();
        assert_eq!(landed.landing(), Some(FloatFormat::F32));
        assert_eq!(landed.value_f32().to_bits(), 1.0f32.to_bits());
        assert_eq!(landed.landed_f64().to_bits(), f64::from(1.0f32).to_bits());
    }
}
