#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub lexeme: String,
}

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

pub fn tokenize(source: &str) -> Vec<Token> {
    source
        .split_whitespace()
        .map(|lexeme| Token {
            lexeme: lexeme.to_owned(),
        })
        .collect()
}
