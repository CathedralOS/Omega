use crate::TokenText;
use crate::{
    CommentKind, FloatLiteralKind, IntegerLiteralKind, KeywordKind, NumericLiteralKind,
    PunctuationKind, Span, TokenKind,
};

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
        matches!(
            self.kind,
            TokenKind::NumericLiteral(NumericLiteralKind::Integer(_))
        )
    }

    pub fn is_float_literal(&self) -> bool {
        matches!(
            self.kind,
            TokenKind::NumericLiteral(NumericLiteralKind::Float(_))
        )
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
