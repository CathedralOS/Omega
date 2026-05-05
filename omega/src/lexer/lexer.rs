use crate::lexer::{Token, TokenKind};

pub fn tokenize(source: &str) -> Vec<Token> {
    source
        .split_whitespace()
        .map(|lexeme| Token {
            kind: TokenKind::Word,
            lexeme: lexeme.to_owned(),
        })
        .collect()
}
