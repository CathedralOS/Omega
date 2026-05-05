pub mod ast;
pub mod backend;
pub mod diagnostics;
pub mod driver;
pub mod hir;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod sema;
pub mod source;
pub mod syntax;

pub use lexer::{Lexer, Token, TokenKind};
pub use syntax::Module;

#[cfg(test)]
mod tests {
    use crate::Lexer;

    #[test]
    fn tokenizes_simple_source() {
        let tokens = Lexer::new("let answer = 42")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].lexeme, "let");
        assert_eq!(tokens[3].lexeme, "42");
    }
}
