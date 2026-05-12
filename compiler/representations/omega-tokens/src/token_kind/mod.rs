mod comment_kind;
mod keyword_kind;
mod numeric_literal_kind;
mod punctuation_kind;
mod whitespace_kind;

pub use comment_kind::CommentKind;
pub use keyword_kind::KeywordKind;
pub use numeric_literal_kind::NumericLiteralKind;
pub use punctuation_kind::PunctuationKind;
pub use whitespace_kind::WhitespaceKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenKind {
    #[default]
    Identifier,
    NumericLiteral(NumericLiteralKind),
    StringLiteral,
    Keyword(KeywordKind),
    Punctuation(PunctuationKind),
    Whitespace(WhitespaceKind),
    Comment(CommentKind),
}
