pub mod ast;
pub mod backend;
pub mod diagnostics;
pub mod driver;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod semantic;
pub mod source;
pub mod syntax;

pub use lexer::{Lexer, Token, TokenKind};
pub use syntax::Module;

#[cfg(test)]
mod tests {
    use crate::Lexer;
    use crate::ast::item::Item;
    use crate::ast::statement::{Statement, TransitionTarget};
    use crate::parser::parser::parse_file;

    #[test]
    fn tokenizes_simple_source() {
        let tokens = Lexer::new("let answer = 42")
            .tokenize()
            .expect("tokenization should succeed");

        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].lexeme, "let");
        assert_eq!(tokens[3].lexeme, "42");
    }

    #[test]
    fn parses_nested_transition_continuation() {
        let tokens = Lexer::new(
            r#"
            machine main {
                state Running {
                    -> dungeon.Main -> Shutdown;
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let Item::Machine(machine) = &parsed.items[0] else {
            panic!("expected a machine");
        };
        let Statement::Transition(transition) = &machine.states[0].statements[0] else {
            panic!("expected a transition");
        };

        assert_eq!(
            transition.target,
            TransitionTarget::Named(vec!["dungeon".to_owned(), "Main".to_owned()])
        );
        assert_eq!(
            transition.continuation,
            Some(TransitionTarget::Named(vec!["Shutdown".to_owned()]))
        );
    }
}
