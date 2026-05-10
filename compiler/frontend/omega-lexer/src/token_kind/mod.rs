mod keyword_kind;
mod punctuation_kind;

pub use keyword_kind::KeywordKind;
pub use punctuation_kind::PunctuationKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenKind {
    #[default]
    Identifier,
    IntegerLiteral,
    FloatLiteral,
    StringLiteral,
    Keyword(KeywordKind),
    Punctuation(PunctuationKind),
}
