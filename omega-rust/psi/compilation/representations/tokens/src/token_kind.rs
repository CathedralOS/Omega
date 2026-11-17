/*
We classify lexical tokens into identifiers, numeric literals, string literals,
keywords (`KeywordKind`), punctuation (`PunctuationKind`), whitespace, and
comments (`CommentKind`), ordering multi-character punctuation lexemes before
their prefixes for deterministic longest-match lexing.
*/

pub(crate) mod comment_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum CommentKind {
        Line,
        #[default]
        Block,
    }
}
pub(crate) mod numeric_literal_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NumericLiteralKind {
        Integer(IntegerLiteralKind),
        Float(FloatLiteralKind),
    }

    impl Default for NumericLiteralKind {
        fn default() -> Self {
            Self::Integer(IntegerLiteralKind::default())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct IntegerLiteralKind {
        pub base: NumericBase,
        pub empty_digits: bool,
        pub has_suffix: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct FloatLiteralKind {
        pub has_exponent: bool,
        pub empty_exponent: bool,
        pub has_suffix: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum NumericBase {
        Binary,
        Octal,
        #[default]
        Decimal,
        Hexadecimal,
    }
}
pub use crate::keywords::KeywordKind;
pub use crate::punctuation::PunctuationKind;
pub use comment_kind::CommentKind;
pub use numeric_literal_kind::{
    FloatLiteralKind, IntegerLiteralKind, NumericBase, NumericLiteralKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenKind {
    #[default]
    Identifier,
    NumericLiteral(NumericLiteralKind),
    StringLiteral,
    Keyword(KeywordKind),
    Punctuation(PunctuationKind),
    Whitespace,
    Comment(CommentKind),
}
