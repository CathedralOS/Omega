/*
We classify lexical tokens into identifiers, numeric literals, string literals,
keywords (`KeywordKind`), punctuation (`PunctuationKind`), whitespace, and
comments (`CommentKind`), ordering multi-character punctuation lexemes before
their prefixes for deterministic longest-match lexing.
*/

pub use super::keywords::KeywordKind;
pub use super::punctuation::PunctuationKind;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    NumericLiteral(NumericLiteralKind),
    StringLiteral,
    Keyword(KeywordKind),
    Punctuation(PunctuationKind),
    Whitespace,
    Comment(CommentKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericLiteralKind {
    Integer(IntegerLiteralKind),
    Float(FloatLiteralKind),
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
