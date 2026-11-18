//! The one error a file can produce, and the closed set of lexical rules it can name.

use crate::span::Span;
use std::fmt;

/// The first lexical rule a source file broke, and the bytes that broke it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}

impl LexError {
    pub fn new(kind: LexErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} (bytes {}..{})",
            self.kind.get_message(),
            self.span.start,
            self.span.end
        )
    }
}

impl std::error::Error for LexError {}

/// Each kind corresponds to a rule in CONTRACT.md. Fixtures match on the kind's name, so the
/// wording of a message can change without touching them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
            Self::NonAsciiOutsideCommentOrQuoted => {
                "a non-ASCII character may appear only inside a comment or a quoted literal"
            }
            Self::UnknownCharacter => "this character is not part of Omega's syntax",
            Self::UnterminatedBlockComment => "block comment is never closed",
            Self::UnterminatedQuoted => "quoted literal is never closed",
            Self::NewlineInQuoted => {
                "a quoted literal cannot contain a raw line break; write \\n or \\r"
            }
            Self::CodepointEscape => {
                "codepoint escapes are not supported; a quoted literal is bytes, so write \\xNN"
            }
            Self::UnknownEscape => {
                "unknown escape; the escapes are \\n \\r \\t \\0 \\\\ \\\" and \\xNN"
            }
            Self::InvalidHexEscape => "\\x must be followed by exactly two hexadecimal digits",
            Self::RawLiteralPrefix => "Omega has no raw literal form",
            Self::MissingRadixDigits => "a radix prefix must be followed by at least one digit",
            Self::DigitOutsideRadix => "this digit is too large for the literal's radix",
            Self::MissingExponentDigits => "an exponent must have at least one digit",
            Self::InvalidLiteralSuffix => {
                "a number may only be followed by i, u, i<width>, u<width>, f32 or f64"
            }
        }
    }
}
