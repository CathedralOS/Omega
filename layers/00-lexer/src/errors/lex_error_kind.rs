//! The closed set of lexical rules a source file can break.

/// Each kind corresponds to a rule in CONTRACT.md. Fixtures match on the kind's name, so the
/// wording of a message can change without touching them.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LexErrorKind {
    InvalidUtf8,
    NonAsciiOutsideCommentOrQuoted,
    UnknownCharacter,
    UnterminatedBlockComment,
    UnterminatedQuoted,
    NewlineInQuoted,
    CodepointEscape,
    UnknownEscape,
    InvalidHexEscape,
    RawLiteralPrefix,
    MissingRadixDigits,
    DigitOutsideRadix,
    MissingExponentDigits,
    InvalidLiteralSuffix,
}

impl LexErrorKind {
    pub fn get_message(self) -> &'static str {
        match self {
            Self::InvalidUtf8 => "source is not valid UTF-8",
            Self::NonAsciiOutsideCommentOrQuoted => "a non-ASCII character may appear only inside a comment or a quoted literal",
            Self::UnknownCharacter => "this character is not part of Omega's syntax",
            Self::UnterminatedBlockComment => "block comment is never closed",
            Self::UnterminatedQuoted => "quoted literal is never closed",
            Self::NewlineInQuoted => "a quoted literal cannot contain a raw line break; write \\n or \\r",
            Self::CodepointEscape => "codepoint escapes are not supported; a quoted literal is bytes, so write \\xNN",
            Self::UnknownEscape => "unknown escape; the escapes are \\n \\r \\t \\0 \\\\ \\\" and \\xNN",
            Self::InvalidHexEscape => "\\x must be followed by exactly two hexadecimal digits",
            Self::RawLiteralPrefix => "Omega has no raw literal form",
            Self::MissingRadixDigits => "a radix prefix must be followed by at least one digit",
            Self::DigitOutsideRadix => "this digit is too large for the literal's radix",
            Self::MissingExponentDigits => "an exponent must have at least one digit",
            Self::InvalidLiteralSuffix => "a number may only be followed by i, u, i<width>, u<width>, f32 or f64",
        }
    }
}
