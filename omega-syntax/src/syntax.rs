use crate::lexer::{Lexer, Token};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Module {
    pub source: String,
    pub tokens: Vec<Token>,
}

impl Module {
    pub fn from_source(source: &str) -> Self {
        Self {
            source: source.to_owned(),
            tokens: Lexer::new(source)
                .tokenize()
                .expect("legacy syntax module tokenization failed"),
        }
    }
}
