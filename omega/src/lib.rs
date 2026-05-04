pub mod syntax;

pub use syntax::{Module, Token, tokenize};

#[cfg(test)]
mod tests {
    use crate::tokenize;

    #[test]
    fn tokenizes_simple_source() {
        let tokens = tokenize("let answer = 42");

        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].lexeme, "let");
        assert_eq!(tokens[3].lexeme, "42");
    }
}
