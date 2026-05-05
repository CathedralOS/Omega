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

    #[test]
    fn parses_platform_command_parameters() {
        let tokens = Lexer::new(
            r#"
            platform Console {
                command ReadLine(mut out_line: ConsoleLine);
                command ExitProcess(return_code: i32);
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let Item::Platform(platform) = &parsed.items[0] else {
            panic!("expected a platform");
        };

        assert_eq!(platform.commands[0].name, "ReadLine");
        assert_eq!(platform.commands[0].parameters[0].name, "out_line");
        assert_eq!(
            platform.commands[0].parameters[0].type_reference.name,
            "ConsoleLine"
        );
        assert!(platform.commands[0].parameters[0].is_mutable);
        assert_eq!(
            platform.commands[1].parameters[0].type_reference.name,
            "i32"
        );
    }

    #[test]
    fn rejects_wrong_platform_argument_count() {
        let tokens = Lexer::new(
            r#"
            platform Console {
                command WriteLine(text: String);
            }

            machine main {
                contains console: Console;

                state Main {
                    console.WriteLine();
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let program =
            crate::ir::lowering::lower_program(&parsed.items).expect("lowering should succeed");
        let diagnostics = crate::semantic::validation::validate_program(&program)
            .expect_err("validation should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expects 1 argument"))
        );
    }

    #[test]
    fn rejects_unknown_contained_type() {
        let tokens = Lexer::new(
            r#"
            machine main {
                contains console: MissingConsole;

                state Main {
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let program =
            crate::ir::lowering::lower_program(&parsed.items).expect("lowering should succeed");
        let diagnostics = crate::semantic::validation::validate_program(&program)
            .expect_err("validation should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unknown type"))
        );
    }
}
