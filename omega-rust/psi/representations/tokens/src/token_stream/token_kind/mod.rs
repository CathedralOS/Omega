//! `TokenKind`: the class the lexer assigns to each token.
//!
//! A token is an identifier, a numeric literal, a string literal, a keyword,
//! punctuation, whitespace, or a comment; `Identifier` is the default. The
//! child modules define the detail four of those classes carry:
//!
//! - `keyword_kind`: `KeywordKind`, and `from_lexeme`, which maps a spelling
//!   to its keyword.
//! - `punctuation_kind`: `PunctuationKind`, and `ordered_lexemes`, which lists
//!   longer spellings before their prefixes so the lexer's first match is the
//!   longest one.
//! - `numeric_literal_kind`: an integer (base, empty digits, suffix) or a
//!   float (exponent, empty exponent, suffix).
//! - `comment_kind`: a line or block comment.

mod comment_kind;
mod keyword_kind;
mod numeric_literal_kind;
mod punctuation_kind;

pub use comment_kind::CommentKind;
pub use keyword_kind::KeywordKind;
pub use numeric_literal_kind::{
    FloatLiteralKind, IntegerLiteralKind, NumericBase, NumericLiteralKind,
};
pub use punctuation_kind::PunctuationKind;

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
