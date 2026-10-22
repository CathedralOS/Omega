use super::DecodedRange;
use crate::{
    CommentKind, FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericLiteralKind,
    PunctuationKind, Span, TokenKind,
};
use arena::Handle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub(super) decoded: Handle<DecodedRange>,
    pub span: Span,
}

impl Token {
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

    pub fn integer_literal_kind(&self) -> Option<IntegerLiteralKind> {
        match self.kind {
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(kind)) => Some(kind),
            _ => None,
        }
    }

    pub fn float_literal_kind(&self) -> Option<FloatLiteralKind> {
        match self.kind {
            TokenKind::NumericLiteral(NumericLiteralKind::Float(kind)) => Some(kind),
            _ => None,
        }
    }

    pub fn is_whitespace(&self) -> bool {
        matches!(self.kind, TokenKind::Whitespace)
    }

    pub fn comment(&self) -> Option<CommentKind> {
        match self.kind {
            TokenKind::Comment(kind) => Some(kind),
            _ => None,
        }
    }

    pub fn is_non_semantic(&self) -> bool {
        matches!(self.kind, TokenKind::Whitespace | TokenKind::Comment(_))
    }
}
