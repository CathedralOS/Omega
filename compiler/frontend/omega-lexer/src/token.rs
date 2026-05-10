use crate::{KeywordKind, PunctuationKind, Span, TokenKind};
use crate::TokenText;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'source> {
    pub kind: TokenKind,
    pub lexeme: TokenText<'source>,
    pub span: Span,
}

impl<'source> Token<'source> {
    pub fn is_identifier(&self) -> bool {
        matches!(self.kind, TokenKind::Identifier)
    }

    pub fn is_integer_literal(&self) -> bool {
        matches!(self.kind, TokenKind::IntegerLiteral)
    }

    pub fn is_float_literal(&self) -> bool {
        matches!(self.kind, TokenKind::FloatLiteral)
    }

    pub fn is_string_literal(&self) -> bool {
        matches!(self.kind, TokenKind::StringLiteral)
    }

    pub fn keyword(&self) -> Option<KeywordKind> {
        match self.kind {
            TokenKind::Keyword(keyword) => Some(keyword),
            _ => None,
        }
    }

    pub fn punctuation(&self) -> Option<PunctuationKind> {
        match self.kind {
            TokenKind::Punctuation(punctuation) => Some(punctuation),
            _ => None,
        }
    }
}
