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
