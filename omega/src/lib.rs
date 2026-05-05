pub mod ast;
pub mod diagnostics;
pub mod driver;
pub mod ir;
pub mod lexer;
pub mod native;
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
    use crate::ast::types::TypeReference;
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
            platform.commands[0].parameters[0].type_reference,
            TypeReference::named("ConsoleLine")
        );
        assert!(platform.commands[0].parameters[0].is_mutable);
        assert_eq!(
            platform.commands[1].parameters[0].type_reference,
            TypeReference::named("i32")
        );
    }

    #[test]
    fn parses_data_variants_and_fields() {
        let tokens = Lexer::new(
            r#"
            data CellId {
                Empty,
                A1,
            }

            data Level {
                rooms: [Room; 16];
                room_count: u32;
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let Item::Data(cell_id) = &parsed.items[0] else {
            panic!("expected data definition");
        };
        let Item::Data(level) = &parsed.items[1] else {
            panic!("expected data definition");
        };

        assert_eq!(cell_id.name, "CellId");
        assert_eq!(cell_id.members.len(), 2);
        assert_eq!(level.name, "Level");
        assert_eq!(level.members.len(), 2);
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

    #[test]
    fn parses_named_and_mutable_command_arguments() {
        let tokens = Lexer::new(
            r#"
            machine main {
                contains console: Console;

                state Main {
                    console.ReadLine(mut input.line);
                    console.ExitProcess(return_code);
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

        let Statement::CommandCall(read_line) = &machine.states[0].statements[0] else {
            panic!("expected command call");
        };
        let crate::ast::expression::Expression::Mutable(inner_expression) = &read_line.arguments[0]
        else {
            panic!("expected mutable argument");
        };
        let crate::ast::expression::Expression::Name(path) = inner_expression.as_ref() else {
            panic!("expected named mutable argument");
        };

        assert_eq!(path, &vec!["input".to_owned(), "line".to_owned()]);
    }

    #[test]
    fn parses_assignment_statement() {
        let tokens = Lexer::new(
            r#"
            machine main {
                state Main {
                    return_code = 0;
                    player.position = next_position;
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
        let Statement::Assignment(first_assignment) = &machine.states[0].statements[0] else {
            panic!("expected assignment");
        };
        let Statement::Assignment(second_assignment) = &machine.states[0].statements[1] else {
            panic!("expected assignment");
        };

        assert_eq!(first_assignment.target, vec!["return_code".to_owned()]);
        assert_eq!(
            second_assignment.target,
            vec!["player".to_owned(), "position".to_owned()]
        );
    }

    #[test]
    fn parses_local_command_call_without_receiver() {
        let tokens = Lexer::new(
            r#"
            machine main {
                state Main {
                    WriteCurrentCell(level, current_cell, mut command_line);
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
        let Statement::CommandCall(command_call) = &machine.states[0].statements[0] else {
            panic!("expected command call");
        };

        assert_eq!(command_call.receiver, None);
        assert_eq!(command_call.command, "WriteCurrentCell");
        assert_eq!(command_call.arguments.len(), 3);
    }

    #[test]
    fn parses_owned_machine_data() {
        let tokens = Lexer::new(
            r#"
            machine main {
                owns return_code: i32 = 0;
                owns current_cell: CellId = CellId::Empty;

                state Main {
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

        assert_eq!(machine.owned_data[0].name, "return_code");
        assert_eq!(machine.owned_data[1].name, "current_cell");
        assert!(machine.owned_data[0].initial_value.is_some());
        assert!(machine.owned_data[1].initial_value.is_some());
    }

    #[test]
    fn plans_native_layout_for_owned_data() {
        let tokens = Lexer::new(
            r#"
            data CellId {
                Empty,
                A1,
            }

            data Player {
                cell: CellId;
                score: u32;
            }

            machine main {
                owns player: Player;

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
        crate::semantic::validation::validate_program(&program).expect("validation should pass");
        let native_plan = crate::native::plan::build_native_plan(
            &program,
            crate::native::target::NativeTarget::host(),
        )
        .expect("native planning should pass");
        let main_layout = native_plan
            .layouts
            .machine_layouts
            .iter()
            .find(|layout| layout.name == "main")
            .expect("main layout should exist");

        assert_eq!(main_layout.fields[0].name, "player");
        assert!(main_layout.layout.size >= 8);
    }
}
