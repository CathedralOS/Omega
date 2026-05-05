use crate::lexer::{Token, tokenize};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Module {
    pub source: String,
    pub tokens: Vec<Token>,
}

impl Module {
    pub fn from_source(source: &str) -> Self {
        Self {
            source: source.to_owned(),
            tokens: tokenize(source),
        }
    }
}
