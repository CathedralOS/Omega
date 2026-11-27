//! Construct retained tokens through the lexer's public entry point.
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
    let mut loaded = LoadedSource {
        source,
        ..Default::default()
    };
    let lexed = lexing::lex_file(&mut loaded, &mut tokens::TokenScratch::default())?;
    Ok(lexed.tokens)
}
