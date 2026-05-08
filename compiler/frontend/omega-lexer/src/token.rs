use crate::{Span, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenText<'source> {
    Source(&'source str),
    Owned(String),
}

impl<'source> TokenText<'source> {
    pub fn source(value: &'source str) -> Self {
        Self::Source(value)
    }

    pub fn owned(value: String) -> Self {
        Self::Owned(value)
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Source(value) => value,
            Self::Owned(value) => value.as_str(),
        }
    }
}

impl PartialEq<&str> for TokenText<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<TokenText<'_>> for &str {
    fn eq(&self, other: &TokenText<'_>) -> bool {
        *self == other.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'source> {
    pub kind: TokenKind,
    pub lexeme: TokenText<'source>,
    pub span: Span,
}
