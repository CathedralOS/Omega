//! The token the lexer produces: a kind and the byte range it covers.

use crate::locations::source_span::SourceSpan;
use crate::tokens::token_kind::TokenKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token {
    kind: TokenKind,
    span: SourceSpan,
}

impl Token {
    pub fn new(
        kind: TokenKind,
        span: SourceSpan,
    ) -> Self {
        Self { kind, span }
    }

    pub fn get_kind(&self) -> TokenKind {
        self.kind
    }

    pub fn get_span(&self) -> SourceSpan {
        self.span
    }
}
