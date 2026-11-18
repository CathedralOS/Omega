//! The token the lexer produces: a kind and the byte range it covers.

use crate::punctuation::Punctuation;
use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// A run of spaces, tabs, carriage returns and line feeds.
    Whitespace,
    /// `//` up to, but not including, the next line feed.
    LineComment,
    /// `/*` through its matching `*/`. Block comments nest.
    BlockComment,
    /// `[A-Za-z_][A-Za-z0-9_]*`. Keywords and names are both words; see the note in `lib.rs`.
    Word,
    /// Digits with no fraction and no exponent, in any radix.
    Integer {
        radix: Radix,
        suffix_start: usize, // Byte offset where the suffix begins. Equals `span.end` when there is none.
    },
    /// Decimal digits with a fraction, an exponent, or both.
    Float {
        suffix_start: usize, // As for Integer.
    },
    /// A quoted byte literal, including both quote characters.
    Quoted,
    Punctuation(Punctuation),
}

impl TokenKind {
    /// Whitespace and comments carry no meaning for a parser; they exist so every byte has a token.
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment
        )
    }
}

/// The base an integer literal's digits are written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Radix {
    Binary,
    Octal,
    Decimal,
    Hexadecimal,
}

impl Radix {
    pub fn get_base(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }

    /// How many bytes the radix prefix (`0x`, `0b`, `0o`) takes. Decimal has none.
    pub fn get_prefix_byte_count(self) -> usize {
        match self {
            Self::Decimal => 0,
            Self::Binary | Self::Octal | Self::Hexadecimal => 2,
        }
    }
}
