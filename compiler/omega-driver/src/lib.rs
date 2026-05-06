pub mod driver;
pub mod ir;
pub mod native;
pub mod proof;
pub mod semantic;

pub(crate) use omega_ast as ast;
pub(crate) use omega_core::{diagnostics, source};
pub(crate) use omega_lexer as lexer;
pub(crate) use omega_parser as parser;

pub use driver::{CheckOutput, CompileOptions, CompileOutput, PhaseTiming, check, compile};

#[cfg(test)]
mod tests {
    use crate::ast::expression::{BinaryOperator, Expression};
    use crate::ast::item::Item;
    use crate::ast::statement::{Statement, TransitionGuard, TransitionTarget};
    use crate::ast::types::{TypeConstraint, TypeReference};
    use crate::parser::parser::parse_file;
    use omega_lexer::Lexer;

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
    fn parse_errors_carry_token_spans() {
        let tokens = Lexer::new(
            r#"
            machine main {
                state entry {
                    let value i32;
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let error = parse_file(&tokens).expect_err("parse should fail");

        assert_eq!(error.message, "expected `:`");
        assert!(error.span.is_some());
    }

    #[test]
    fn source_files_map_offsets_to_line_columns() {
        let file = crate::source::SourceFile {
            id: crate::source::FileId(0),
            path: "sample.omg".into(),
            source: "alpha\nbeta".to_owned(),
        };

        assert_eq!(file.position_at(0).line, 1);
        assert_eq!(file.position_at(0).column, 1);
        assert_eq!(file.position_at(6).line, 2);
        assert_eq!(file.position_at(6).column, 1);
    }

    #[test]
    fn parses_nested_transition_continuation() {
        let tokens = Lexer::new(
            r#"
            machine main {
                state running {
                    -> dungeon.entry -> shutdown;
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
            TransitionTarget::Named {
                path: vec!["dungeon".to_owned(), "entry".to_owned()],
                arguments: Vec::new(),
            }
        );
        assert_eq!(
            transition.continuation,
            Some(TransitionTarget::Named {
                path: vec!["shutdown".to_owned()],
                arguments: Vec::new(),
            })
        );
    }

    #[test]
    fn parses_platform_state_parameters() {
        let tokens = Lexer::new(
            r#"
            platform Console {
                state read_line(mut out_line: ConsoleLine);
                state exit_process(return_code: i32);
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let Item::Platform(platform) = &parsed.items[0] else {
            panic!("expected a platform");
        };

        assert_eq!(platform.states[0].name, "read_line");
        assert_eq!(platform.states[0].parameters[0].name, "out_line");
        assert_eq!(
            platform.states[0].parameters[0].type_reference,
            TypeReference::named("ConsoleLine")
        );
        assert!(platform.states[0].parameters[0].is_mutable);
        assert_eq!(
            platform.states[1].parameters[0].type_reference,
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
                state write_line(text: String);
            }

            machine main {
                contains console: Console;

                state entry {
                    console.write_line();
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
    fn rejects_duplicate_local_data_in_state_body() {
        let tokens = Lexer::new(
            r#"
            machine main {
                state bad_locals() {
                    let value: i32;
                    let value: i32;
                }

                state entry {
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
                .any(|diagnostic| diagnostic.message.contains("duplicate local data `value`"))
        );
    }

    #[test]
    fn rejects_duplicate_platform_states_and_parameters() {
        let tokens = Lexer::new(
            r#"
            platform Console {
                state write_line(text: String);
                state write_line(mut text: String);
                state echo(text: String, text: String);
            }

            machine main {
                contains console: Console;

                state entry {
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
                .any(|diagnostic| diagnostic.message.contains("duplicate state"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("duplicate parameter"))
        );
    }

    #[test]
    fn rejects_unknown_contained_type() {
        let tokens = Lexer::new(
            r#"
            machine main {
                contains console: MissingConsole;

                state entry {
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
    fn rejects_duplicate_machine_local_names() {
        let tokens = Lexer::new(
            r#"
            platform Console {
                state write_line(text: String);
            }

            machine main {
                contains console: Console;
                contains console: Console;

                state entry {
                }

                state entry {
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
                .any(|diagnostic| diagnostic.message.contains("duplicate contained object"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("duplicate state"))
        );
    }

    #[test]
    fn rejects_duplicate_machine_members_across_contains_and_owns() {
        let tokens = Lexer::new(
            r#"
            data Value {
                raw: i32;
            }

            platform Console {
                state write_line(text: String);
            }

            machine main {
                contains output: Console;
                owns output: Value;

                state entry {
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
                .any(|diagnostic| diagnostic.message.contains("duplicate member `output`"))
        );
    }

    #[test]
    fn rejects_duplicate_data_members() {
        let tokens = Lexer::new(
            r#"
            data Broken {
                value: i32;
                value: u32;
            }

            machine main {
                state entry {
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
                .any(|diagnostic| diagnostic.message.contains("duplicate member"))
        );
    }

    #[test]
    fn rejects_mixed_data_shapes() {
        let tokens = Lexer::new(
            r#"
            data Confused {
                Ready,
                value: i32;
            }

            machine main {
                state entry {
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
                .any(|diagnostic| diagnostic.message.contains("mixes fields and variants"))
        );
    }

    #[test]
    fn rejects_empty_data_definition() {
        let tokens = Lexer::new(
            r#"
            data Empty {
            }

            machine main {
                state entry {
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
                .any(|diagnostic| diagnostic.message.contains("must declare at least one"))
        );
    }

    #[test]
    fn rejects_machine_type_as_owned_data() {
        let tokens = Lexer::new(
            r#"
            machine Worker {
                state entry {
                }
            }

            machine main {
                owns worker: Worker;

                state entry {
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
                .any(|diagnostic| diagnostic.message.contains("unknown data type `Worker`"))
        );
    }

    #[test]
    fn parses_named_and_mutable_call_arguments() {
        let tokens = Lexer::new(
            r#"
            machine main {
                contains console: Console;

                state entry {
                    console.read_line(mut input.line);
                    console.exit_process(return_code);
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

        let Statement::Call(read_line) = &machine.states[0].statements[0] else {
            panic!("expected state call");
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
                state entry {
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

        assert_eq!(
            first_assignment.target,
            Expression::Name(vec!["return_code".to_owned()])
        );
        assert_eq!(
            second_assignment.target,
            Expression::Name(vec!["player".to_owned(), "position".to_owned()])
        );
    }

    #[test]
    fn parses_local_call_without_receiver() {
        let tokens = Lexer::new(
            r#"
            machine main {
                state entry {
                    write_current_cell(level, current_cell, mut command_line);
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
        let Statement::Call(call) = &machine.states[0].statements[0] else {
            panic!("expected state call");
        };

        assert_eq!(call.receiver, None);
        assert_eq!(call.target, "write_current_cell");
        assert_eq!(call.arguments.len(), 3);
    }

    #[test]
    fn parses_owned_machine_data() {
        let tokens = Lexer::new(
            r#"
            machine main {
                owns return_code: i32 = 0;
                owns current_cell: CellId = CellId::Empty;

                state entry {
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

                state entry {
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
        let main_fields = native_plan
            .layouts
            .fields
            .span(main_layout.fields)
            .expect("main fields should resolve");

        assert_eq!(main_fields[0].name, "player");
        assert!(main_layout.layout.size >= 8);
    }

    #[test]
    fn plans_native_layout_for_primitive_widths() {
        let tokens = Lexer::new(
            r#"
            data Counters {
                slot: usize;
                label: String;
            }

            machine main {
                owns counters: Counters;

                state entry {
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
        let target = crate::native::target::NativeTarget::host();
        let native_plan = crate::native::plan::build_native_plan(&program, target)
            .expect("native planning should pass");
        let counters_layout = native_plan
            .layouts
            .data_layouts
            .iter()
            .find(|layout| layout.name == "Counters")
            .expect("Counters layout should exist");
        let crate::native::layout::DataShape::Record { fields } = &counters_layout.shape else {
            panic!("expected record layout");
        };
        let fields = native_plan
            .layouts
            .fields
            .span(*fields)
            .expect("counter fields should resolve");

        assert_eq!(fields[0].layout.size, target.pointer_size);
        assert_eq!(fields[1].layout.size, target.pointer_size * 2);
    }

    #[test]
    fn parses_state_body_statements() {
        let tokens = Lexer::new(
            r#"
            machine Sample {
                state write(mut out_level: Level) {
                    let room: Room;
                    out_level.rooms[0] = Room { cell: CellId::A1 };
                    out_level.name = "A" + "1";
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

        assert_eq!(machine.states[0].name, "write");
        assert_eq!(machine.states[0].statements.len(), 3);
    }

    #[test]
    fn parses_terminal_completion_arrow() {
        let tokens = Lexer::new(
            r#"
            machine Sample {
                state done {
                    ->
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
            panic!("expected terminal transition");
        };

        assert_eq!(transition.target, TransitionTarget::Terminal);
        assert_eq!(transition.continuation, None);
        assert_eq!(transition.guard, TransitionGuard::Always);
    }

    #[test]
    fn parses_typed_state_final_expression_and_self_parameter() {
        let tokens = Lexer::new(
            r#"
            machine Math {
                state clamp_done(&mut self, value: f32) -> f32 {
                    value
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

        assert_eq!(
            machine.states[0].return_type,
            Some(TypeReference::named("f32"))
        );
        assert!(machine.states[0].parameters[0].is_self);
        assert_eq!(machine.states[0].parameters[1].name, "value");

        let Statement::Expression(Expression::Name(path)) = &machine.states[0].statements[0] else {
            panic!("expected final expression");
        };
        assert_eq!(path, &vec!["value".to_owned()]);
    }

    #[test]
    fn parses_transition_arguments_and_guarded_terminal_completion() {
        let tokens = Lexer::new(
            r#"
            machine Math {
                state clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
                    -> self.clamp_low(min) when value < min;
                    -> when value == min
                }

                state clamp_low(&mut self, min: f32) -> f32 {
                    min
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

        let Statement::Transition(first_transition) = &machine.states[0].statements[0] else {
            panic!("expected transition");
        };
        let TransitionTarget::Named { path, arguments } = &first_transition.target else {
            panic!("expected named transition target");
        };
        assert_eq!(path, &vec!["self".to_owned(), "clamp_low".to_owned()]);
        assert_eq!(arguments.len(), 1);

        let Statement::Transition(second_transition) = &machine.states[0].statements[1] else {
            panic!("expected terminal transition");
        };
        assert_eq!(second_transition.target, TransitionTarget::Terminal);
        let TransitionGuard::When(Expression::Binary(condition)) = &second_transition.guard else {
            panic!("expected structured transition guard");
        };
        assert_eq!(condition.operator, BinaryOperator::Equal);
    }

    #[test]
    fn parses_const_parameters_and_bounded_types() {
        let tokens = Lexer::new(
            r#"
            machine Math {
                state clamp(value: i32, min: const i32, max: const i32) -> i32[range<min, max>] {
                    value
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
        let TypeReference::Named(min_type) = &machine.states[0].parameters[1].type_reference else {
            panic!("expected named const parameter type");
        };
        let TypeReference::Constrained {
            base_type,
            constraints,
        } = machine.states[0]
            .return_type
            .as_ref()
            .expect("return type should exist")
        else {
            panic!("expected constrained return type");
        };

        assert!(machine.states[0].parameters[1].is_const);
        assert_eq!(min_type, "i32");
        assert_eq!(base_type.as_ref(), &TypeReference::named("i32"));
        let [TypeConstraint::Range { minimum, maximum }] = constraints.as_slice() else {
            panic!("expected one range constraint");
        };
        let Expression::Name(minimum_path) = minimum else {
            panic!("expected range minimum name");
        };
        let Expression::Name(maximum_path) = maximum else {
            panic!("expected range maximum name");
        };
        assert_eq!(minimum_path, &vec!["min".to_owned()]);
        assert_eq!(maximum_path, &vec!["max".to_owned()]);
    }

    #[test]
    fn parses_targets_capabilities_and_generic_types() {
        let tokens = Lexer::new(
            r#"
            target local_unchecked {
                host: StandardHost {
                    stdout = enabled
                    filesystem = sandbox("./")
                }

                trust host_contracts
                trust unchecked invariant_proofs
            }

            capability Stdout {
                state write(buf: Slice<u8>) -> Result<(), IOError>
                    requires buf.initialized
                    ensures result.Ok
                    trusted host
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let Item::Target(target) = &parsed.items[0] else {
            panic!("expected a target");
        };
        let Item::Capability(capability) = &parsed.items[1] else {
            panic!("expected a capability");
        };

        assert_eq!(target.name, "local_unchecked");
        assert_eq!(
            target
                .host
                .as_ref()
                .expect("host should exist")
                .settings
                .len(),
            2
        );
        assert_eq!(target.trust_policies.len(), 2);

        let crate::ast::item::CapabilityMember::State(state) = &capability.members[0] else {
            panic!("expected capability state");
        };
        assert_eq!(state.contracts.len(), 3);

        let TypeReference::Generic {
            base_name,
            arguments,
        } = &state.signature.parameters[0].type_reference
        else {
            panic!("expected generic parameter type");
        };
        assert_eq!(base_name, "Slice");
        assert_eq!(arguments, &vec![TypeReference::named("u8")]);

        let TypeReference::Generic {
            base_name,
            arguments,
        } = state
            .signature
            .return_type
            .as_ref()
            .expect("return type should exist")
        else {
            panic!("expected generic return type");
        };
        assert_eq!(base_name, "Result");
        assert_eq!(
            arguments,
            &vec![TypeReference::Unit, TypeReference::named("IOError")]
        );
    }

    #[test]
    fn builds_trust_report_from_targets_and_capabilities() {
        let tokens = Lexer::new(
            r#"
            target local_unchecked {
                host: StandardHost {
                    stdout = enabled
                }

                trust host_contracts
                trust unchecked invariant_proofs
            }

            capability Process {
                state exit(return_code: i32) -> Never
                    requires target_accepts_exit_code(return_code)
                    ensures process_terminated
                    trusted host
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let trust_report = crate::driver::trust::build_trust_report(&parsed.items, None);

        assert_eq!(trust_report.targets.len(), 1);
        assert_eq!(trust_report.trusted_contracts.len(), 1);
        assert_eq!(trust_report.unchecked_policies.len(), 1);

        let (_, target) = trust_report
            .targets
            .iter()
            .next()
            .expect("target should exist");
        assert_eq!(target.host_provider, "StandardHost");
        assert_eq!(target.checked_trusts, 1);
        assert_eq!(target.unchecked_trusts, 1);

        let (_, contract) = trust_report
            .trusted_contracts
            .iter()
            .next()
            .expect("trusted contract should exist");
        assert_eq!(contract.capability, "Process");
        assert_eq!(contract.state, "exit");
        assert_eq!(contract.trust_level, "host");
        assert_eq!(contract.requires_count, 1);
        assert_eq!(contract.ensures_count, 1);
    }

    #[test]
    fn expands_invariant_aliases_during_lowering() {
        let tokens = Lexer::new(
            r#"
            invariant finite_value = [finite];
            invariant speed_range = [finite_value, range<0.0f, 100000.0f>];

            machine main {
                owns speed: f32[speed_range] = 0.0f;

                state entry {
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let program =
            crate::ir::lowering::lower_program(&parsed.items).expect("lowering should succeed");
        let invariant_constraints = program
            .type_constraints
            .span(program.invariant_definitions[1].constraints)
            .expect("invariant constraints should resolve");
        let crate::ir::types::TypeReference::Constrained {
            base_type,
            constraints,
        } = &program.machines[0].owned_data[0].type_reference
        else {
            panic!("expected constrained owned data");
        };

        assert_eq!(
            base_type.as_ref(),
            &crate::ir::types::TypeReference::Named("f32".to_owned())
        );
        let owned_data_constraints = program
            .type_constraints
            .span(*constraints)
            .expect("owned data constraints should resolve");
        assert_eq!(owned_data_constraints.len(), 2);
        assert_eq!(invariant_constraints, owned_data_constraints);
        assert!(matches!(
            owned_data_constraints[0],
            crate::ir::types::TypeConstraint::Named(ref name) if name == "finite"
        ));
        assert!(matches!(
            owned_data_constraints[1],
            crate::ir::types::TypeConstraint::Range { .. }
        ));
    }

    #[test]
    fn plans_state_control_flow() {
        let tokens = Lexer::new(
            r#"
            machine main {
                owns return_code: i32 = 0;

                state entry {
                    let temp: i32;
                    return_code = 1;
                    -> running;
                }

                state running {
                    -> self;
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
        let control_flow = crate::native::control_flow::build_control_flow_plan(&program)
            .expect("control-flow planning should pass");
        let states = control_flow
            .states
            .span(control_flow.machines[0].states)
            .expect("machine states should resolve");
        let entry_operations = control_flow
            .operations
            .span(states[0].operations)
            .expect("entry operations should resolve");

        assert_eq!(entry_operations[0].statement_index, 0);
        assert_eq!(entry_operations[1].statement_index, 1);
        assert_eq!(states[0].transitions.len(), 1);
        assert_eq!(states[1].transitions.len(), 1);
    }

    #[test]
    fn plans_mid_state_transition_as_generated_segments() {
        let tokens = Lexer::new(
            r#"
            machine main {
                owns ready: bool = false;

                state entry {
                    -> done when ready;
                    prepare();
                    -> done;
                }

                state prepare {
                }

                state done {
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
        let control_flow = crate::native::control_flow::build_control_flow_plan(&program)
            .expect("control-flow planning should pass");
        let states = control_flow
            .states
            .span(control_flow.machines[0].states)
            .expect("machine states should resolve");

        assert_eq!(states[0].name, "entry");
        assert_eq!(states[1].name, "entry__segment_1");
        assert_eq!(states[0].transitions.len(), 2);
        assert_eq!(states[1].operations.len(), 1);
        assert_eq!(states[1].transitions.len(), 1);
    }

    #[test]
    fn builds_proof_obligations_for_bounds_and_guards() {
        let tokens = Lexer::new(
            r#"
            machine main {
                owns health: i32[range<1, 100>] = 100;

                state entry {
                    -> damaged(health) when health > 50;
                    -> done;
                }

                state damaged(amount: i32[range<1, 50>]) {
                }

                state done {
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
        let proof_plan = crate::proof::obligations::build_proof_plan(&program);

        assert_eq!(proof_plan.obligations.len(), 5);
        assert!(proof_plan.obligations.iter().any(|obligation| {
            matches!(
                obligation,
                crate::proof::obligations::ProofObligation::BoundedInitializer(
                    initializer_obligation
                ) if initializer_obligation.owner == "machine `main` owned data `health`"
            )
        }));
        assert!(proof_plan.obligations.iter().any(|obligation| {
            matches!(
                obligation,
                crate::proof::obligations::ProofObligation::BoundedTransitionArgument(
                    transition_obligation
                ) if transition_obligation.parameter == "amount"
            )
        }));
    }

    #[test]
    fn plans_native_object_shape() {
        let tokens = Lexer::new(
            r#"
            machine main {
                owns return_code: i32 = 0;

                state entry {
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

        assert_eq!(native_plan.object.sections.len(), 3);
        assert!(
            native_plan
                .object
                .symbols
                .iter()
                .any(|(_, symbol)| symbol.name.contains("main"))
        );
    }

    #[test]
    fn selected_windows_target_plans_coff_and_kernel32_imports() {
        let tokens = Lexer::new(
            r#"
            platform Console {
                state write_line(text: String);
                state exit_process(return_code: i32);
            }

            machine main {
                contains console: Console;

                state entry {
                    console.write_line("Hello.");
                    console.exit_process(0);
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let program =
            crate::ir::lowering::lower_program(&parsed.items).expect("lowering should succeed");
        let native_plan = crate::native::plan::build_native_plan(
            &program,
            crate::native::target::NativeTarget::windows_x64(),
        )
        .expect("native planning should pass");

        assert_eq!(
            native_plan.target.object_format,
            crate::native::target::ObjectFormat::Coff
        );
        assert!(
            native_plan
                .object
                .symbols
                .iter()
                .any(|(_, symbol)| symbol.name == "WriteFile")
        );
        assert!(
            native_plan
                .host_abi
                .bindings
                .iter()
                .any(|(_, binding)| binding.trust_policy == "omega::host::windows")
        );
        assert_eq!(native_plan.host_calls.calls.len(), 2);
        assert_eq!(native_plan.host_calls.operations.len(), 3);
    }

    #[test]
    fn selected_linux_target_plans_elf_and_syscalls() {
        let tokens = Lexer::new(
            r#"
            platform Console {
                state write_line(text: String);
                state exit_process(return_code: i32);
            }

            machine main {
                contains console: Console;

                state entry {
                    console.write_line("Hello.");
                    console.exit_process(0);
                }
            }
            "#,
        )
        .tokenize()
        .expect("tokenization should succeed");
        let parsed = parse_file(&tokens).expect("parse should succeed");
        let program =
            crate::ir::lowering::lower_program(&parsed.items).expect("lowering should succeed");
        let native_plan = crate::native::plan::build_native_plan(
            &program,
            crate::native::target::NativeTarget::linux_x64(),
        )
        .expect("native planning should pass");

        assert_eq!(
            native_plan.target.object_format,
            crate::native::target::ObjectFormat::Elf
        );
        assert_eq!(native_plan.object.symbols.len(), 2);
        assert!(
            native_plan
                .host_abi
                .bindings
                .iter()
                .any(|(_, binding)| binding.operation == "exit_group")
        );
        assert_eq!(native_plan.host_calls.calls.len(), 2);
        assert_eq!(native_plan.host_calls.operations.len(), 2);
    }

    #[test]
    fn check_writes_phase_artifacts() {
        let build_dir =
            std::env::temp_dir().join(format!("omega-driver-artifacts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&build_dir);
        let root_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/cli_mvp/main.omg");

        let output = crate::check(crate::CompileOptions {
            build_dir: Some(build_dir.clone()),
            root_path,
            target_name: None,
        })
        .expect("check should pass");

        for file_name in [
            "00_timings.txt",
            "01_sources.txt",
            "02_ast.txt",
            "03_resolve.txt",
            "04_types.txt",
            "05_driver_ir.txt",
            "06_validation.txt",
            "07_graph.txt",
            "08_proof.txt",
            "09_native_plan.txt",
            "10_trust.txt",
            "11_emission.txt",
        ] {
            assert!(
                output.artifacts_dir.join(file_name).is_file(),
                "missing artifact {file_name}"
            );
        }
        assert!(!output.phase_timings.is_empty());

        let sources = std::fs::read_to_string(output.artifacts_dir.join("01_sources.txt"))
            .expect("source artifact should be readable");
        assert!(
            sources.contains("omega/std/console.omg"),
            "source artifact should include bundled omega std source"
        );
        let emission = std::fs::read_to_string(output.artifacts_dir.join("11_emission.txt"))
            .expect("emission artifact should be readable");
        assert!(emission.contains("status: blocked before byte emission"));

        let _ = std::fs::remove_dir_all(build_dir);
    }

    #[test]
    fn selected_target_loads_only_referenced_host_package() {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-driver-selected-target-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&build_dir);
        let root_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/cli_mvp/main.omg");

        let output = crate::check(crate::CompileOptions {
            build_dir: Some(build_dir.clone()),
            root_path,
            target_name: Some("windows_x64".to_owned()),
        })
        .expect("check should pass");
        let sources = std::fs::read_to_string(output.artifacts_dir.join("01_sources.txt"))
            .expect("source artifact should be readable");

        assert!(sources.contains("omega/host/windows.omg"));
        assert!(sources.contains("omega/host/contracts.omg"));
        assert!(!sources.contains("omega/host/linux.omg"));
        assert!(!sources.contains("omega/host/darwin.omg"));

        let trust = std::fs::read_to_string(output.artifacts_dir.join("10_trust.txt"))
            .expect("trust artifact should be readable");
        assert!(trust.contains("targets: 1"));
        assert!(trust.contains("target `windows_x64`"));
        assert!(!trust.contains("target `linux_x64`"));
        assert!(!trust.contains("unchecked `invariant_proofs`"));

        let _ = std::fs::remove_dir_all(build_dir);
    }

    #[test]
    fn checks_every_sample_entrypoint() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("driver crate should live under compiler/omega-driver");
        let sample_root = repo_root.join("samples");
        let mut entrypoints = Vec::new();

        collect_entrypoints(&sample_root, &mut entrypoints);
        entrypoints.sort();

        assert!(
            !entrypoints.is_empty(),
            "expected at least one sample entrypoint under {}",
            sample_root.display()
        );

        for root_path in entrypoints {
            let build_dir = std::env::temp_dir().join(format!(
                "omega-sample-check-{}-{}",
                std::process::id(),
                root_path
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or("sample")
            ));
            let _ = std::fs::remove_dir_all(&build_dir);

            crate::check(crate::CompileOptions {
                build_dir: Some(build_dir.clone()),
                root_path: root_path.clone(),
                target_name: None,
            })
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "sample {} failed check:\n{}",
                    root_path.display(),
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });

            let _ = std::fs::remove_dir_all(build_dir);
        }
    }

    #[test]
    fn checks_passing_canaries() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("driver crate should live under compiler/omega-driver");
        let canary_root = repo_root.join("canaries/pass");
        let mut entrypoints = Vec::new();

        collect_entrypoints(&canary_root, &mut entrypoints);
        entrypoints.sort();

        assert!(
            !entrypoints.is_empty(),
            "expected at least one passing canary under {}",
            canary_root.display()
        );

        for root_path in entrypoints {
            let build_dir = temporary_build_dir("omega-canary-pass", &root_path);
            let _ = std::fs::remove_dir_all(&build_dir);

            crate::check(crate::CompileOptions {
                build_dir: Some(build_dir.clone()),
                root_path: root_path.clone(),
                target_name: None,
            })
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "passing canary {} failed check:\n{}",
                    root_path.display(),
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });

            let _ = std::fs::remove_dir_all(build_dir);
        }
    }

    #[test]
    fn rejects_failing_canaries() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("driver crate should live under compiler/omega-driver");
        let canary_root = repo_root.join("canaries/fail");
        let mut entrypoints = Vec::new();

        collect_entrypoints(&canary_root, &mut entrypoints);
        entrypoints.sort();

        assert!(
            !entrypoints.is_empty(),
            "expected at least one failing canary under {}",
            canary_root.display()
        );

        for root_path in entrypoints {
            let build_dir = temporary_build_dir("omega-canary-fail", &root_path);
            let _ = std::fs::remove_dir_all(&build_dir);

            let expected_diagnostic_path = root_path
                .parent()
                .expect("canary entrypoint should have a parent directory")
                .join("expected.txt");
            let expected_diagnostic = std::fs::read_to_string(&expected_diagnostic_path)
                .unwrap_or_else(|error| {
                    panic!(
                        "failed to read expected diagnostic {}: {error}",
                        expected_diagnostic_path.display()
                    )
                });
            let diagnostics = crate::check(crate::CompileOptions {
                build_dir: Some(build_dir.clone()),
                root_path: root_path.clone(),
                target_name: None,
            })
            .expect_err(&format!(
                "failing canary {} unexpectedly passed",
                root_path.display()
            ));
            let diagnostics_text = diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");

            assert!(
                diagnostics_text.contains(expected_diagnostic.trim()),
                "failing canary {} expected diagnostic containing `{}`, got:\n{}",
                root_path.display(),
                expected_diagnostic.trim(),
                diagnostics_text
            );

            let _ = std::fs::remove_dir_all(build_dir);
        }
    }

    fn collect_entrypoints(path: &std::path::Path, entrypoints: &mut Vec<std::path::PathBuf>) {
        let entries = std::fs::read_dir(path)
            .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", path.display()));

        for entry in entries {
            let entry =
                entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}"));
            let path = entry.path();

            if path.is_dir() {
                collect_entrypoints(&path, entrypoints);
            } else if path
                .file_name()
                .is_some_and(|file_name| file_name == "main.omg")
            {
                entrypoints.push(path);
            }
        }
    }

    fn temporary_build_dir(prefix: &str, root_path: &std::path::Path) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            root_path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("entrypoint")
        ))
    }
}
