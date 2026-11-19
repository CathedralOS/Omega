//! What a token is: the closed set of shapes the lexer can cut out of a source file.

use crate::tokens::integer_radix::IntegerRadix;
use crate::tokens::punctuation::Punctuation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        radix: IntegerRadix,
        suffix_start_offset: usize, // Byte offset where the suffix begins. Equals the token's end when there is none.
    },

    /// Decimal digits with a fraction, an exponent, or both.
    Float {
        suffix_start_offset: usize, // As for Integer.
    },

    /// A quoted byte literal, including both quote characters.
    Quoted,

    Punctuation(Punctuation),
}

impl TokenKind {
    /// Whitespace and comments carry no meaning for a parser; they exist so every byte has a token.
    pub fn is_trivia(self) -> bool {
        matches!(self, Self::Whitespace | Self::LineComment | Self::BlockComment)
    }

    /// The kind's name without the details a number or punctuation carries, for dumps and reports.
    pub fn get_name(self) -> &'static str {
        match self {
            Self::Whitespace => "Whitespace",
            Self::LineComment => "LineComment",
            Self::BlockComment => "BlockComment",
            Self::Word => "Word",
            Self::Integer { .. } => "Integer",
            Self::Float { .. } => "Float",
            Self::Quoted => "Quoted",
            Self::Punctuation(_) => "Punctuation",
        }
    }
}
