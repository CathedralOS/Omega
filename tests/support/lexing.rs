//! Construct retained tokens through the batch lexer's public entry point.
use super::{LoadedSource, lexing};
use std::sync::Arc;

pub struct Lexer<'text>(&'text str);
impl<'text> Lexer<'text> {
    pub fn new(text: &'text str) -> Self {
        Self(text)
    }
    pub fn tokenize(self) -> Result<tokens::TokenStream<'static>, lexing::LexError> {
        lex_retained(Arc::from(self.0))
    }
}

pub fn lex_retained(source: Arc<str>) -> Result<tokens::TokenStream<'static>, lexing::LexError> {
    let loaded = [LoadedSource {
        source,
        ..Default::default()
    }]
    .into_iter()
    .collect();
    let mut lexed = lexing::lex(loaded)?;
    Ok(std::mem::take(
        &mut lexed.sources.span_mut_or_empty(lexed.batch)[0].tokens,
    ))
}
