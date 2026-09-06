//! The current source token sequence and its spelling vocabulary.

pub mod token;
pub mod token_kind;
pub mod token_text;

use std::ops::Deref;

use crate::Token;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TokenStream<'source> {
    tokens: Vec<Token<'source>>,
}

impl<'source> TokenStream<'source> {
    pub fn new(tokens: Vec<Token<'source>>) -> Self {
        Self { tokens }
    }

    pub fn as_slice(&self) -> &[Token<'source>] {
        &self.tokens
    }

    pub fn into_tokens(self) -> Vec<Token<'source>> {
        self.tokens
    }
}

impl<'source> Deref for TokenStream<'source> {
    type Target = [Token<'source>];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}
