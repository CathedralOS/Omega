use crate::ast::expression::{BinaryOperator, Expression};
use crate::ast::item::Item;
use crate::ast::statement::{Statement, TransitionGuard, TransitionTarget};
use crate::ast::types::{TypeConstraint, TypeReference};
use crate::parser::parser::parse_file;
use omega_lexer::Lexer;
use omega_typed_program::lowering::lower_program;

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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan =
        omega_native::plan::build_native_plan(&program, omega_native::target::NativeTarget::host())
            .expect("native planning should pass");
    let main_layout = native_plan
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, layout)| layout.name == "main")
        .map(|(_, layout)| layout)
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let target = omega_native::target::NativeTarget::host();
    let native_plan = omega_native::plan::build_native_plan(&program, target)
        .expect("native planning should pass");
    let counters_layout = native_plan
        .layouts
        .data_layouts
        .iter()
        .find(|(_, layout)| layout.name == "Counters")
        .map(|(_, layout)| layout)
        .expect("Counters layout should exist");
    let omega_native::layout::DataShape::Record { fields } = &counters_layout.shape else {
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
                state write(buf: Slice<u8>) -> Result<(), IOError> {
                    requires buf.initialized
                    ensures result.Ok
                    trust host
                }
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
fn parses_top_level_trust_definitions() {
    let tokens = Lexer::new(
        r#"
            trust omega_windows_kernel32 {
                owner omega::host::targets::windows
                reason "Windows Kernel32 API contract"
                scope {
                    dll "Kernel32.dll"
                    symbols GetStdHandle, WriteFile, ExitProcess
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let Item::TrustDefinition(trust_definition) = &parsed.items[0] else {
        panic!("expected a trust definition");
    };

    assert_eq!(trust_definition.name, "omega_windows_kernel32");
    assert!(trust_definition.token_count > 0);
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
                state exit(return_code: i32) -> Terminal {
                    requires target_accepts_exit_code(return_code)
                    ensures process_terminated
                    trust host
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let trust_report = crate::pipeline::trust::build_trust_report(&parsed.items, None);

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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let invariant_constraints = program
        .type_constraints
        .span(program.invariant_definitions[1].constraints)
        .expect("invariant constraints should resolve");
    let omega_typed_program::types::TypeReference::Constrained {
        base_type,
        constraints,
    } = &program.machines[0].owned_data[0].type_reference
    else {
        panic!("expected constrained owned data");
    };

    assert_eq!(
        base_type.as_ref(),
        &omega_typed_program::types::TypeReference::Named("f32".to_owned())
    );
    let owned_data_constraints = program
        .type_constraints
        .span(*constraints)
        .expect("owned data constraints should resolve");
    assert_eq!(owned_data_constraints.len(), 2);
    assert_eq!(invariant_constraints, owned_data_constraints);
    assert!(matches!(
        owned_data_constraints[0],
        omega_typed_program::types::TypeConstraint::Named(ref name) if name == "finite"
    ));
    assert!(matches!(
        owned_data_constraints[1],
        omega_typed_program::types::TypeConstraint::Range { .. }
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let control_flow = omega_native::control_flow::build_control_flow_plan(&program)
        .expect("control-flow planning should pass");
    let (_, machine) = control_flow
        .machines
        .iter()
        .next()
        .expect("machine should exist");
    let states = control_flow
        .states
        .span(machine.states)
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
fn plans_runtime_state_flow_without_rejecting_cycles() {
    let tokens = Lexer::new(
        r#"
            machine main {
                state entry {
                    -> prompt;
                }

                state prompt {
                    -> invalid_command;
                }

                state invalid_command {
                    -> prompt;
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should preserve runtime loops");
    let runtime_states = native_plan
        .runtime_flow
        .states
        .iter()
        .map(|(_, state)| format!("{}.{}", state.machine, state.state))
        .collect::<Vec<_>>();

    assert_eq!(
        runtime_states,
        vec!["main.entry", "main.prompt", "main.invalid_command",]
    );
    assert_eq!(native_plan.runtime_flow.edges.len(), 3);
    assert_eq!(native_plan.runtime_flow.cycles.len(), 1);
    assert!(
        native_plan
            .runtime_flow
            .edges
            .iter()
            .any(|(_, edge)| edge.forms_cycle),
        "expected the prompt loop to be represented as a cycle edge"
    );
}

#[test]
fn reports_runtime_dispatch_blockers_for_state_cycles() {
    let tokens = Lexer::new(
        r#"
            machine main {
                owns ready: bool = false;

                state entry {
                    -> prompt;
                }

                state prompt {
                    -> done when ready;
                    -> invalid_command;
                }

                state invalid_command {
                    -> prompt;
                }

                state done {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should preserve runtime loops");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);

    assert!(
        emission_plan.blockers.iter().any(|(_, blocker)| {
            blocker.stage == "runtime dispatch"
                && blocker
                    .reason
                    .contains("runtime state comparison byte emission")
        }),
        "expected guarded dispatch loop emission blocker"
    );
}

#[test]
fn emits_dispatch_control_bytes_for_unguarded_state_cycles() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state write_line(text: String);
            }

            machine main {
                contains console: Console;

                state entry {
                    -> prompt;
                }

                state prompt {
                    console.write_line("prompt");
                    -> invalid_command;
                }

                state invalid_command {
                    console.write_line("invalid");
                    -> prompt;
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should emit unguarded dispatch loop bytes");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);

    assert!(
        !emission_plan
            .blockers
            .iter()
            .any(|(_, blocker)| blocker.stage == "runtime dispatch"),
        "unguarded dispatch loops should not report a runtime dispatch blocker"
    );
    assert!(
        native_plan
            .machine_code
            .instructions
            .iter()
            .any(|(_, instruction)| {
                matches!(
                    instruction.kind,
                    omega_native::machine_code::MachineInstructionKind::DispatchCaseEnter { .. }
                ) && instruction.byte_width == 8
                    && instruction.bytes.count() == 8
            })
    );
    assert!(
        native_plan
            .machine_code
            .instructions
            .iter()
            .any(|(_, instruction)| {
                instruction.kind
                    == omega_native::machine_code::MachineInstructionKind::DispatchCaseLeave
                    && instruction.byte_width == 4
                    && instruction.bytes.count() == 4
            })
    );
}

#[test]
fn plans_runtime_dispatch_indices_for_state_cycles() {
    let tokens = Lexer::new(
        r#"
            machine main {
                state entry {
                    -> prompt;
                }

                state prompt {
                    -> invalid_command;
                }

                state invalid_command {
                    -> prompt;
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should build dispatch data");
    let prompt = native_plan
        .state_dispatch
        .states
        .iter()
        .find(|(_, state)| state.machine == "main" && state.state == "prompt")
        .map(|(_, state)| state)
        .expect("prompt dispatch state should exist");
    let prompt_edges = native_plan
        .state_dispatch
        .edges
        .span(prompt.edges)
        .expect("prompt edges should resolve");

    assert_eq!(prompt.dispatch_index, 2);
    assert_eq!(prompt.label, "omega_state_main_prompt");
    assert_eq!(prompt_edges.len(), 1);
    assert_eq!(prompt_edges[0].target_dispatch_index, 3);
}

#[test]
fn plans_runtime_dispatch_loop_for_state_cycles() {
    let tokens = Lexer::new(
        r#"
            machine main {
                owns ready: bool = false;

                state entry {
                    -> prompt;
                }

                state prompt {
                    -> done when ready == false;
                    -> invalid_command;
                }

                state invalid_command {
                    -> prompt;
                }

                state done {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should build dispatch loop data");
    let prompt_case = native_plan
        .runtime_dispatch_loop
        .cases
        .iter()
        .find(|(_, dispatch_case)| dispatch_case.state == "prompt")
        .map(|(_, dispatch_case)| dispatch_case)
        .expect("prompt dispatch loop case should exist");
    let prompt_edges = native_plan
        .runtime_dispatch_loop
        .edges
        .span(prompt_case.edges)
        .expect("prompt dispatch loop edges should resolve");

    assert!(native_plan.runtime_dispatch_loop.needed);
    assert_eq!(native_plan.runtime_dispatch_loop.entry_dispatch_index, 1);
    assert_eq!(native_plan.runtime_dispatch_loop.terminal_dispatch_index, 0);
    assert_eq!(
        native_plan.runtime_dispatch_loop.current_state_slot,
        "omega_current_state"
    );
    assert_eq!(prompt_edges.len(), 2);
    assert!(prompt_edges.iter().any(|edge| {
        edge.action
            == omega_native::runtime_dispatch::loop_plan::RuntimeDispatchLoopAction::EnterState
            && edge.target_dispatch_index != 0
    }));
    assert!(prompt_edges.iter().any(|edge| {
        edge.guard_lowering == omega_native::state_guards::StateGuardLowering::CompareStaticValue
    }));
}

#[test]
fn selects_runtime_dispatch_loop_instructions() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state write_line(text: String);
            }

            machine main {
                contains console: Console;

                state entry {
                    -> prompt;
                }

                state prompt {
                    console.write_line("prompt");
                    -> invalid_command;
                }

                state invalid_command {
                    console.write_line("invalid");
                    -> prompt;
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should select dispatch loop instructions");

    assert!(
        native_plan
            .instructions
            .instructions
            .iter()
            .any(|(_, instruction)| {
                matches!(
                    instruction.kind,
                    omega_native::instructions::SelectedInstructionKind::EnterDispatchLoop { .. }
                )
            })
    );
    assert!(
        native_plan
            .instructions
            .instructions
            .iter()
            .any(|(_, instruction)| {
                matches!(
                    instruction.kind,
                    omega_native::instructions::SelectedInstructionKind::EnterDispatchCase {
                        ref label,
                        ..
                    } if label == "omega_state_main_prompt"
                )
            })
    );
    assert!(
        native_plan
            .instructions
            .instructions
            .iter()
            .any(|(_, instruction)| {
                matches!(
                    instruction.kind,
                    omega_native::instructions::SelectedInstructionKind::SetDispatchState { .. }
                )
            })
    );
    assert_eq!(native_plan.host_calls.calls.len(), 2);
    assert!(
        native_plan
            .instructions
            .instructions
            .iter()
            .any(|(_, instruction)| {
                matches!(
                    instruction.kind,
                    omega_native::instructions::SelectedInstructionKind::HostOperation { .. }
                )
            })
    );
}

#[test]
fn plans_runtime_guards_for_dispatch_edges() {
    let tokens = Lexer::new(
        r#"
            machine main {
                owns ready: bool = false;

                state entry {
                    -> done when ready == true;
                    -> self;
                }

                state done {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should build guard data");

    assert_eq!(native_plan.state_guards.guards.len(), 2);
    assert!(native_plan.state_guards.guards.iter().any(|(_, guard)| {
        guard.source_machine == "main"
            && guard.source_state == "entry"
            && guard.kind == omega_native::state_guards::StateGuardKind::RuntimeEquality
            && guard.operator == omega_native::state_guards::StateGuardOperator::Equal
            && guard.lowering == omega_native::state_guards::StateGuardLowering::CompareStaticValue
            && guard.expression.display_name() == "ready == true"
    }));
    let equality_guard = native_plan
        .state_guards
        .guards
        .iter()
        .find(|(_, guard)| {
            guard.source_machine == "main"
                && guard.source_state == "entry"
                && guard.kind == omega_native::state_guards::StateGuardKind::RuntimeEquality
        })
        .map(|(_, guard)| guard)
        .expect("runtime equality guard should be planned");
    let guard_operands = native_plan
        .state_guards
        .operands
        .span(equality_guard.operands)
        .expect("runtime guard operands should resolve");

    assert_eq!(guard_operands.len(), 2);
    assert_eq!(guard_operands[0].expression.display_name(), "ready");
    assert_eq!(
        guard_operands[0].kind,
        omega_native::state_guards::StateGuardOperandKind::Place
    );
    assert_eq!(
        guard_operands[0].storage,
        omega_native::state_guards::StateGuardOperandStorage::MachineOwned
    );
    assert_eq!(guard_operands[0].byte_offset, 0);
    assert_eq!(guard_operands[0].byte_size, 1);
    assert_eq!(guard_operands[1].expression.display_name(), "true");
    assert_eq!(
        guard_operands[1].kind,
        omega_native::state_guards::StateGuardOperandKind::Literal
    );
    assert!(guard_operands[1].has_resolved_value);
    assert_eq!(guard_operands[1].resolved_value, 1);
    assert!(native_plan.state_guards.guards.iter().any(|(_, guard)| {
        guard.source_machine == "main"
            && guard.source_state == "entry"
            && guard.kind == omega_native::state_guards::StateGuardKind::Always
            && !guard.has_expression
            && guard.forms_cycle
    }));
}

#[test]
fn resolves_enum_guard_operand_values() {
    let tokens = Lexer::new(
        r#"
            data Choice {
                Quit,
                Look,
            }

            machine main {
                owns choice: Choice = Choice::Quit;

                state entry {
                    -> done when choice == Choice::Look;
                    -> self;
                }

                state done {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should resolve enum guard operands");
    let equality_guard = native_plan
        .state_guards
        .guards
        .iter()
        .find(|(_, guard)| {
            guard.source_machine == "main"
                && guard.source_state == "entry"
                && guard.kind == omega_native::state_guards::StateGuardKind::RuntimeEquality
        })
        .map(|(_, guard)| guard)
        .expect("runtime equality guard should be planned");
    let guard_operands = native_plan
        .state_guards
        .operands
        .span(equality_guard.operands)
        .expect("runtime guard operands should resolve");

    assert_eq!(guard_operands.len(), 2);
    assert_eq!(guard_operands[1].expression.display_name(), "Choice::Look");
    assert_eq!(
        guard_operands[1].kind,
        omega_native::state_guards::StateGuardOperandKind::StaticSymbol
    );
    assert!(guard_operands[1].has_resolved_value);
    assert_eq!(guard_operands[1].resolved_value, 1);
}

#[test]
fn resolves_nested_guard_operand_offsets() {
    let tokens = Lexer::new(
        r#"
            data Choice {
                Quit,
                Look,
            }

            data Navigation {
                choice: Choice;
                destination: Choice;
            }

            machine main {
                owns navigation: Navigation;

                state entry {
                    -> done when navigation.destination == Choice::Look;
                    -> self;
                }

                state done {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should resolve nested guard operand offsets");
    let equality_guard = native_plan
        .state_guards
        .guards
        .iter()
        .find(|(_, guard)| {
            guard.source_machine == "main"
                && guard.source_state == "entry"
                && guard.kind == omega_native::state_guards::StateGuardKind::RuntimeEquality
        })
        .map(|(_, guard)| guard)
        .expect("runtime equality guard should be planned");
    let guard_operands = native_plan
        .state_guards
        .operands
        .span(equality_guard.operands)
        .expect("runtime guard operands should resolve");

    assert_eq!(
        guard_operands[0].expression.display_name(),
        "navigation::destination"
    );
    assert_eq!(
        guard_operands[0].storage,
        omega_native::state_guards::StateGuardOperandStorage::MachineOwned
    );
    assert_eq!(guard_operands[0].byte_offset, 4);
    assert_eq!(guard_operands[0].byte_size, 4);
    assert_eq!(guard_operands[1].resolved_value, 1);
}

#[test]
fn plans_runtime_bodies_with_leaf_state_call_expansion() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state write_line(text: String);
            }

            machine main {
                contains console: Console;

                state entry {
                    hello();
                    -> self;
                }

                state hello {
                    console.write_line("body");
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should build runtime bodies");
    let entry_body = native_plan
        .runtime_bodies
        .bodies
        .iter()
        .find(|(_, body)| body.machine == "main" && body.state == "entry")
        .map(|(_, body)| body)
        .expect("entry runtime body should exist");
    let operations = native_plan
        .runtime_bodies
        .operations
        .span(entry_body.operations)
        .expect("entry runtime body operations should resolve");

    assert!(operations.iter().any(|operation| matches!(
        operation.kind,
        omega_native::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind::InlineLeafStateCall { ref target_state, .. }
            if target_state == "hello"
    )));
    assert!(operations.iter().any(|operation| matches!(
        operation.kind,
        omega_native::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind::HostCall { ref platform_call }
            if platform_call == "console.write_line"
    )));
}

#[test]
fn plans_runtime_branching_state_call_edges() {
    let tokens = Lexer::new(
        r#"
            machine main {
                owns ready: bool = false;

                state entry {
                    choose();
                    -> self;
                }

                state choose {
                    -> yes when ready;
                    -> no;
                }

                state yes {
                }

                state no {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should build runtime branching calls");
    let branching_call = native_plan
        .runtime_branching_calls
        .calls
        .iter()
        .find(|(_, call)| call.target_state == "choose")
        .map(|(_, call)| call)
        .expect("branching helper call should be planned");
    let edges = native_plan
        .runtime_branching_calls
        .edges
        .span(branching_call.edges)
        .expect("branching call edges should resolve");

    assert_eq!(
        branching_call.expansion,
        omega_native::runtime_dispatch::branching::RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards
    );
    assert_eq!(native_plan.runtime_branching_calls.leaf_expansions.len(), 2);
    assert_eq!(edges.len(), 2);
    assert!(matches!(
        edges[0].target,
        omega_native::runtime_flow::RuntimeTransitionTarget::State { ref state, .. }
            if state == "yes"
    ));
    assert_eq!(
        edges[0].lowering,
        omega_native::runtime_dispatch::branching::RuntimeBranchTargetLowering::InlineLeaf
    );
    assert_eq!(
        edges[0].guard_kind,
        omega_native::state_guards::StateGuardKind::RuntimeExpression
    );
    assert!(matches!(
        edges[1].target,
        omega_native::runtime_flow::RuntimeTransitionTarget::State { ref state, .. }
            if state == "no"
    ));
    assert_eq!(
        edges[1].lowering,
        omega_native::runtime_dispatch::branching::RuntimeBranchTargetLowering::InlineLeaf
    );
    assert_eq!(
        edges[1].guard_kind,
        omega_native::state_guards::StateGuardKind::Always
    );
}

#[test]
fn skips_state_call_blocker_for_planned_guarded_leaf_expansion() {
    let tokens = Lexer::new(
        r#"
            machine main {
                owns ready: bool = false;

                state entry {
                    choose();
                    -> self;
                }

                state choose {
                    -> yes when ready == true;
                    ->
                }

                state yes {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should build runtime branch expansion");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);
    let branching_call = native_plan
        .runtime_branching_calls
        .calls
        .iter()
        .find(|(_, call)| call.target_state == "choose")
        .map(|(_, call)| call)
        .expect("branching call should be planned");

    assert_eq!(
        branching_call.expansion,
        omega_native::runtime_dispatch::branching::RuntimeBranchCallExpansion::GuardedLeaf
    );
    assert_eq!(native_plan.runtime_branching_calls.leaf_expansions.len(), 1);
    assert!(
        !emission_plan.blockers.iter().any(|(_, blocker)| {
            blocker.stage == "state calls"
                && blocker.reason.contains("main.entry")
                && blocker.reason.contains("main.choose")
                && blocker.reason.contains("guarded leaf branch expansion")
        }),
        "planned guarded leaf expansion should not report a stale state-call blocker"
    );
}

#[test]
fn plans_runtime_straight_line_branch_expansion() {
    let tokens = Lexer::new(
        r#"
            machine main {
                owns ready: bool = false;
                owns selected: i32 = 0;

                state entry {
                    choose(ready, mut selected);
                    -> self;
                }

                state choose(flag: bool, mut out_selected: i32) {
                    -> yes(mut out_selected) when flag == true;
                    -> fallback(flag, mut out_selected);
                }

                state yes(mut out_selected: i32) {
                    out_selected = 1;
                }

                state fallback(flag: bool, mut out_selected: i32) {
                    apply_default(mut out_selected);
                }

                state apply_default(mut out_selected: i32) {
                    out_selected = 2;
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should build straight-line branch expansion");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);
    let branching_call = native_plan
        .runtime_branching_calls
        .calls
        .iter()
        .find(|(_, call)| call.target_state == "choose")
        .map(|(_, call)| call)
        .expect("branching call should be planned");
    let expansion = native_plan
        .runtime_branching_calls
        .straight_line_expansions
        .iter()
        .find(|(_, expansion)| expansion.target_state == "fallback")
        .map(|(_, expansion)| expansion)
        .expect("fallback straight-line expansion should be planned");
    let bindings = native_plan
        .runtime_branching_calls
        .straight_line_bindings
        .span(expansion.bindings)
        .expect("straight-line branch bindings should resolve");
    let operations = native_plan
        .runtime_branching_calls
        .straight_line_operations
        .span(expansion.operations)
        .expect("straight-line branch operations should resolve");

    assert_eq!(
        branching_call.expansion,
        omega_native::runtime_dispatch::branching::RuntimeBranchCallExpansion::NeedsStraightLineTarget
    );
    assert!(bindings.iter().any(|binding| {
        binding.kind
            == omega_native::runtime_dispatch::branching::RuntimeStraightLineBranchBindingKind::TargetParameter
            && binding.parameter_name == "out_selected"
            && binding.expression.display_name() == "mut selected"
    }));
    assert!(operations.iter().any(|operation| matches!(
        operation.kind,
        omega_native::runtime_dispatch::branching::RuntimeStraightLineBranchOperationKind::StateCall {
            ref target_state,
            lowering: omega_native::state_calls::StateCallLowering::InlineLeaf,
            ..
        } if target_state == "apply_default"
    )));
    assert!(
        native_plan
            .instructions
            .instructions
            .iter()
            .any(|(_, instruction)| matches!(
                instruction.kind,
                omega_native::instructions::SelectedInstructionKind::WriteRuntimeMachineInteger {
                    value: 2,
                    ..
                }
            ) && instruction.source_state == "apply_default"),
        "straight-line branch target should emit its nested leaf mutation"
    );
    assert!(
        !emission_plan.blockers.iter().any(|(_, blocker)| {
            blocker.stage == "state calls"
                && blocker.reason.contains("main.entry")
                && blocker.reason.contains("main.choose")
                && blocker
                    .reason
                    .contains("guarded branch expansion with straight-line target")
        }),
        "planned straight-line branch expansion should not report a stale state-call blocker"
    );
}

#[test]
fn plans_runtime_leaf_branch_argument_bindings() {
    let tokens = Lexer::new(
        r#"
            data CellId {
                Empty,
                A1,
                A2,
            }

            data Cell {
                id: CellId;
            }

            machine main {
                owns selected: Cell;
                owns first: Cell = Cell { id: CellId::A1 };

                state entry {
                    wrapper(first, CellId::A1, mut selected);
                    -> self;
                }

                state wrapper(cell: Cell, id: CellId, mut final_cell: Cell) {
                    scan(cell, id, mut final_cell);
                }

                state scan(cell: Cell, id: CellId, mut out_cell: Cell) {
                    -> apply(cell, mut out_cell) when cell.id == id;
                    ->
                }

                state apply(cell: Cell, mut out_cell: Cell) {
                    out_cell = cell;
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should build leaf branch bindings");
    let expansion = native_plan
        .runtime_branching_calls
        .leaf_expansions
        .iter()
        .find(|(_, expansion)| expansion.leaf_state == "apply")
        .map(|(_, expansion)| expansion)
        .expect("apply leaf expansion should be planned");
    let bindings = native_plan
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .expect("leaf branch bindings should resolve");

    assert_eq!(
        match &expansion.resolved_guard {
            omega_typed_program::statement::TransitionGuard::When(expression) =>
                expression.display_name(),
            omega_typed_program::statement::TransitionGuard::Always => "always".to_owned(),
        },
        "first::id == CellId::A1"
    );
    assert!(bindings.iter().any(|binding| {
        binding.kind
            == omega_native::runtime_dispatch::branching::RuntimeLeafBranchBindingKind::BranchParameter
            && binding.parameter_name == "cell"
            && binding.expression.display_name() == "first"
    }));
    assert!(bindings.iter().any(|binding| {
        binding.kind
            == omega_native::runtime_dispatch::branching::RuntimeLeafBranchBindingKind::LeafParameter
            && binding.parameter_name == "out_cell"
            && binding.expression.display_name() == "mut selected"
    }));
}

#[test]
fn selects_instructions_for_runtime_reachable_loop_states() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state write_line(text: String);
            }

            machine main {
                contains console: Console;

                state entry {
                    -> prompt;
                }

                state prompt {
                    console.write_line("prompt");
                    -> invalid_command;
                }

                state invalid_command {
                    console.write_line("invalid");
                    -> prompt;
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should select reachable loop-state calls");
    let selected_host_operations = native_plan
        .instructions
        .instructions
        .iter()
        .filter(|(_, instruction)| {
            matches!(
                instruction.kind,
                omega_native::instructions::SelectedInstructionKind::HostOperation { .. }
            )
        })
        .count();

    assert_eq!(native_plan.host_calls.calls.len(), 2);
    assert_eq!(selected_host_operations, 2);
}

#[test]
fn selects_host_calls_inside_required_state_call_targets() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state write_line(text: String);
            }

            machine Helper {
                contains console: Console;

                state print {
                    console.write_line("helper");
                }
            }

            machine main {
                contains helper: Helper;

                state entry {
                    -> prompt;
                }

                state prompt {
                    helper.print();
                    -> prompt;
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should select required helper host calls");
    let selected_host_operations = native_plan
        .instructions
        .instructions
        .iter()
        .filter(|(_, instruction)| {
            matches!(
                instruction.kind,
                omega_native::instructions::SelectedInstructionKind::HostOperation { .. }
            )
        })
        .count();

    assert_eq!(native_plan.host_calls.calls.len(), 1);
    assert_eq!(selected_host_operations, 1);
}

#[test]
fn plans_state_calls_separately_from_host_calls() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state write_line(text: String);
            }

            machine Helper {
                state write {
                }
            }

            machine main {
                contains console: Console;
                contains helper: Helper;

                state entry {
                    prepare(1);
                    helper.write();
                    console.write_line("done");
                }

                state prepare(value: i32) {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should separate state calls");
    let state_calls = native_plan
        .state_calls
        .calls
        .iter()
        .map(|(_, state_call)| {
            (
                state_call.source_machine.as_str(),
                state_call.source_state.as_str(),
                state_call.target_machine.as_str(),
                state_call.target_state.as_str(),
                state_call.argument_count,
                state_call.required,
            )
        })
        .collect::<Vec<_>>();
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);

    assert_eq!(
        state_calls,
        vec![
            ("main", "entry", "main", "prepare", 1, true),
            ("main", "entry", "Helper", "write", 0, true),
        ]
    );
    let prepare_call = native_plan
        .state_calls
        .calls
        .iter()
        .find(|(_, state_call)| state_call.target_state == "prepare")
        .map(|(_, state_call)| state_call)
        .expect("prepare state call should be planned");
    let prepare_arguments = native_plan
        .state_calls
        .arguments
        .span(prepare_call.arguments)
        .expect("prepare call arguments should resolve");

    assert_eq!(prepare_arguments.len(), 1);
    assert_eq!(prepare_arguments[0].parameter_name, "value");
    assert_eq!(
        prepare_arguments[0].kind,
        omega_native::state_calls::StateCallArgumentKind::Value
    );
    assert_eq!(
        prepare_call.lowering,
        omega_native::state_calls::StateCallLowering::InlineLeaf
    );
    assert_eq!(native_plan.host_calls.calls.len(), 1);
    assert!(
        !emission_plan
            .blockers
            .iter()
            .any(|(_, blocker)| blocker.stage == "state calls"),
        "acyclic state calls should be flattened by the static state schedule"
    );
}

#[test]
fn marks_state_calls_required_through_transition_targets() {
    let tokens = Lexer::new(
        r#"
            machine Helper {
                state write {
                }
            }

            machine main {
                contains helper: Helper;

                state entry {
                    decide();
                }

                state decide {
                    -> branch;
                }

                state branch {
                    helper.write();
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should trace required calls through transitions");
    let branch_call = native_plan
        .state_calls
        .calls
        .iter()
        .find(|(_, state_call)| {
            state_call.source_machine == "main" && state_call.source_state == "branch"
        })
        .map(|(_, state_call)| state_call)
        .expect("branch state call should be planned");

    assert!(branch_call.required);
    assert!(!branch_call.reachable);
}

#[test]
fn tracks_mutable_state_call_argument_bindings() {
    let tokens = Lexer::new(
        r#"
            data Line {
                text: String;
            }

            machine main {
                owns line: Line;

                state entry {
                    fill(mut line);
                }

                state fill(mut out_line: Line) {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should collect mutable call argument binding");
    let state_call = native_plan
        .state_calls
        .calls
        .iter()
        .next()
        .map(|(_, state_call)| state_call)
        .expect("state call should be planned");
    let arguments = native_plan
        .state_calls
        .arguments
        .span(state_call.arguments)
        .expect("state call arguments should resolve");

    assert_eq!(arguments[0].parameter_name, "out_line");
    assert_eq!(
        arguments[0].kind,
        omega_native::state_calls::StateCallArgumentKind::MutableAlias
    );
    assert!(arguments[0].required);
    let alias = native_plan
        .alias_flow
        .aliases
        .iter()
        .next()
        .map(|(_, alias)| alias)
        .expect("mutable argument should produce an alias binding");

    assert_eq!(alias.parameter_name, "out_line");
    assert_eq!(alias.argument.display_name(), "mut line");
    assert!(alias.required);
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let control_flow = omega_native::control_flow::build_control_flow_plan(&program)
        .expect("control-flow planning should pass");
    let (_, machine) = control_flow
        .machines
        .iter()
        .next()
        .expect("machine should exist");
    let states = control_flow
        .states
        .span(machine.states)
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan =
        omega_native::plan::build_native_plan(&program, omega_native::target::NativeTarget::host())
            .expect("native planning should pass");

    assert_eq!(native_plan.object.sections.len(), 3);
    assert!(!native_plan.instructions.functions.is_empty());
    assert!(!native_plan.instructions.instructions.is_empty());
    assert!(native_plan.machine_code.byte_count > 0);
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::windows_x64(),
    )
    .expect("native planning should pass");

    assert_eq!(
        native_plan.target.object_format,
        omega_native::target::ObjectFormat::Coff
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
            .any(|(_, binding)| binding.trust_policy == "omega::host::targets::windows")
    );
    assert_eq!(native_plan.host_calls.calls.len(), 2);
    assert_eq!(native_plan.host_calls.operations.len(), 3);
    assert_eq!(native_plan.host_calls.arguments.len(), 2);
    assert_eq!(native_plan.data.objects.len(), 1);
    assert_eq!(native_plan.data.bytes.len(), 7);
    assert!(native_plan.instructions.instructions.len() >= 5);
    assert_eq!(native_plan.instructions.operands.len(), 4);
    assert!(native_plan.machine_code.byte_count > 0);
    assert_eq!(native_plan.relocations.records.len(), 4);
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::linux_x64(),
    )
    .expect("native planning should pass");

    assert_eq!(
        native_plan.target.object_format,
        omega_native::target::ObjectFormat::Elf
    );
    assert_eq!(native_plan.object.symbols.len(), 3);
    assert!(
        native_plan
            .host_abi
            .bindings
            .iter()
            .any(|(_, binding)| binding.operation == "exit_group")
    );
    assert_eq!(native_plan.host_calls.calls.len(), 2);
    assert_eq!(native_plan.host_calls.operations.len(), 2);
    assert_eq!(native_plan.data.objects.len(), 1);
    assert_eq!(native_plan.data.bytes.len(), 7);
    assert!(native_plan.instructions.instructions.len() >= 4);
    assert_eq!(native_plan.instructions.operands.len(), 4);
    assert!(native_plan.machine_code.byte_count > 0);
    assert_eq!(native_plan.relocations.records.len(), 1);
}

#[test]
fn selected_macos_arm64_plans_relocation_byte_offsets() {
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
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should pass");

    let relocations = native_plan
        .relocations
        .records
        .iter()
        .map(|(_, relocation)| {
            (
                relocation.kind,
                relocation.text_offset,
                relocation.byte_width,
                relocation.symbol.as_str(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        relocations,
        vec![
            (
                omega_native::relocations::RelocationKind::Aarch64Page21,
                4,
                4,
                "omega_string_literal_1",
            ),
            (
                omega_native::relocations::RelocationKind::Aarch64PageOffset12,
                8,
                4,
                "omega_string_literal_1",
            ),
            (
                omega_native::relocations::RelocationKind::Aarch64Branch26,
                16,
                4,
                "_write",
            ),
            (
                omega_native::relocations::RelocationKind::Aarch64Branch26,
                24,
                4,
                "_exit",
            ),
        ]
    );
}

#[test]
fn encodes_aarch64_immediates_that_need_movk() {
    let long_text = "a".repeat(70_000);
    let source = format!(
        r#"
            platform Console {{
                state write_line(text: String);
            }}

            machine main {{
                contains console: Console;

                state entry {{
                    console.write_line("{long_text}");
                }}
            }}
            "#
    );
    let tokens = Lexer::new(&source)
        .tokenize()
        .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should encode multi-instruction immediates");

    assert_eq!(native_plan.machine_code.byte_count, 28);
}

#[test]
fn reports_platform_calls_without_native_lowering_as_emission_blockers() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state write_error(text: String);
            }

            machine main {
                contains console: Console;

                state entry {
                    console.write_error("nope");
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should preserve unsupported call as blocker");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);

    assert!(
        emission_plan.blockers.iter().any(|(_, blocker)| blocker
            .reason
            .contains("platform call `console.write_error`: no native lowering")),
        "expected unsupported platform call blocker"
    );
}

#[test]
fn check_writes_phase_artifacts() {
    let build_dir =
        std::env::temp_dir().join(format!("omega-compiler-artifacts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    let root_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/cli_mvp/main.omg");

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
        "05_typed_program.txt",
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
        sources.contains("omega/language/std/console.omg"),
        "source artifact should include bundled omega std source"
    );
    let emission = std::fs::read_to_string(output.artifacts_dir.join("11_emission.txt"))
        .expect("emission artifact should be readable");
    let native_plan = std::fs::read_to_string(output.artifacts_dir.join("09_native_plan.txt"))
        .expect("native plan artifact should be readable");
    assert!(native_plan.contains("## Runtime Text"));
    assert!(emission.contains("status: ready to emit"));
    assert!(emission.contains("data bytes:"));
    assert!(emission.contains("selected instructions:"));
    assert!(emission.contains("instruction operands:"));
    assert!(emission.contains("machine code bytes:"));
    assert!(emission.contains("encoded machine bytes:"));
    assert!(emission.contains("relocations:"));

    let _ = std::fs::remove_dir_all(build_dir);
}

#[test]
fn compile_emits_native_object_bytes() {
    let build_dir =
        std::env::temp_dir().join(format!("omega-compiler-compile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);
    let root_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/cli_mvp/main.omg");

    let output = crate::compile(crate::CompileOptions {
        build_dir: Some(build_dir.clone()),
        root_path,
        target_name: Some("macos_arm64".to_owned()),
    })
    .expect("compile should emit bytes");

    assert!(output.executable_path.is_file());
    assert!(output.artifacts_dir.join("12_emitted_object.txt").is_file());
    assert!(output.artifacts_dir.join("13_link.txt").is_file());

    let bytes =
        std::fs::read(&output.executable_path).expect("emitted executable should be readable");
    assert!(bytes.starts_with(&0xfeedfacfu32.to_le_bytes()));
    assert!(bytes.len() > 32);
    assert!(
        output.summary.contains("emitted"),
        "compile summary should report emitted output"
    );
    assert!(
        output.summary.contains("linked"),
        "compile summary should report linked output"
    );

    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        let run_output = std::process::Command::new(&output.executable_path)
            .output()
            .expect("compiled Omega binary should run");

        assert!(
            run_output.status.success(),
            "compiled Omega binary should exit successfully"
        );
        assert_eq!(
            String::from_utf8_lossy(&run_output.stdout),
            "Hello, Omega.\n"
        );
        assert!(
            run_output.stderr.is_empty(),
            "compiled Omega binary should not write stderr"
        );
    }

    let _ = std::fs::remove_dir_all(build_dir);
}

#[test]
fn compile_emits_static_inline_state_call() {
    let build_dir =
        std::env::temp_dir().join(format!("omega-compiler-inline-call-{}", std::process::id()));
    let root_dir = build_dir.join("project");
    let root_path = root_dir.join("main.omg");
    let build_path = root_dir.join("build.omg");
    let _ = std::fs::remove_dir_all(&build_dir);
    std::fs::create_dir_all(&root_dir).expect("test project dir should be creatable");
    std::fs::write(
        build_path,
        r#"
            target macos_arm64 {
                host: omega::host::targets::darwin {
                    abi = libSystem
                    stdout = fd(1)
                    process = enabled
                }

                trust omega::host::contracts
                trust omega::host::targets::darwin
            }
            "#,
    )
    .expect("test build policy should be writable");
    std::fs::write(
        &root_path,
        r#"
            platform Console {
                state write_line(text: String);
                state exit_process(return_code: i32);
            }

            machine main {
                contains console: Console;

                state entry {
                    hello();
                    console.exit_process(0);
                }

                state hello {
                    console.write_line("Inline state call.");
                }
            }
            "#,
    )
    .expect("test source should be writable");

    let output = crate::compile(crate::CompileOptions {
        build_dir: Some(build_dir.join("build")),
        root_path,
        target_name: Some("macos_arm64".to_owned()),
    })
    .expect("compile should emit static inline state call");

    assert!(output.executable_path.is_file());

    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        let run_output = std::process::Command::new(&output.executable_path)
            .output()
            .expect("compiled Omega binary should run");

        assert!(
            run_output.status.success(),
            "compiled Omega binary should exit successfully"
        );
        assert_eq!(
            String::from_utf8_lossy(&run_output.stdout),
            "Inline state call.\n"
        );
    }

    let _ = std::fs::remove_dir_all(build_dir);
}

#[test]
fn compile_rejects_emission_blockers() {
    let build_dir =
        std::env::temp_dir().join(format!("omega-compiler-blocked-{}", std::process::id()));
    let root_dir = build_dir.join("blocked_project");
    let root_path = root_dir.join("main.omg");
    std::fs::create_dir_all(&root_dir).expect("test project dir should be creatable");
    std::fs::write(
        &root_path,
        r#"
            platform Console {
                state write_error(text: String);
            }

            machine main {
                contains console: Console;

                state entry {
                    console.write_error("nope");
                }
            }
            "#,
    )
    .expect("test source should be writable");

    let diagnostics = crate::compile(crate::CompileOptions {
        build_dir: Some(root_dir.join("build")),
        root_path,
        target_name: None,
    })
    .expect_err("compile should reject unresolved emission blockers");
    let diagnostics_text = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        diagnostics_text.contains("cannot emit native binary; host lowering"),
        "unexpected diagnostics:\n{}",
        diagnostics_text
    );

    let _ = std::fs::remove_dir_all(build_dir);
}

#[test]
fn compile_rejects_unknown_native_target_names() {
    let build_dir = std::env::temp_dir().join(format!(
        "omega-compiler-unknown-target-{}",
        std::process::id()
    ));
    let root_dir = build_dir.join("unknown_target_project");
    let root_path = root_dir.join("main.omg");
    std::fs::create_dir_all(&root_dir).expect("test project dir should be creatable");
    std::fs::write(
        root_dir.join("build.omg"),
        r#"
            target weird_box {
                host: omega::host::standard {
                    stdout = enabled
                    process = enabled
                }

                trust omega::host::standard
            }
            "#,
    )
    .expect("test build policy should be writable");
    std::fs::write(
        &root_path,
        r#"
            platform Console {
                state write_line(text: String);
            }

            machine main {
                contains console: Console;

                state entry {
                    console.write_line("Hello.");
                }
            }
            "#,
    )
    .expect("test source should be writable");

    let diagnostics = crate::compile(crate::CompileOptions {
        build_dir: Some(root_dir.join("build")),
        root_path,
        target_name: Some("weird_box".to_owned()),
    })
    .expect_err("compile should reject unknown native target names");
    let diagnostics_text = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        diagnostics_text.contains("unknown native target `weird_box`"),
        "unexpected diagnostics:\n{}",
        diagnostics_text
    );

    let _ = std::fs::remove_dir_all(build_dir);
}

#[test]
fn ignores_host_calls_outside_entry_schedule() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state write_line(text: String);
                state exit_process(return_code: i32);
            }

            machine main {
                contains console: Console;

                state entry {
                    console.exit_process(0);
                }

                state later {
                    console.write_line("not yet");
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should keep unreachable host call out of the schedule");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);

    assert!(emission_plan.blockers.is_empty());
    assert_eq!(native_plan.host_calls.calls.len(), 2);
    assert_eq!(native_plan.instructions.instructions.len(), 4);
}

#[test]
fn emits_unconditional_entry_transition_chains() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state write_line(text: String);
                state exit_process(return_code: i32);
            }

            machine main {
                contains console: Console;

                state entry {
                    console.write_line("entry");

                    -> shutdown;
                }

                state shutdown {
                    console.write_line("shutdown");
                    console.exit_process(0);
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should allow unconditional transition chain");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);

    assert!(emission_plan.blockers.is_empty());
    assert_eq!(native_plan.host_calls.calls.len(), 3);
    assert_eq!(native_plan.instructions.instructions.len(), 8);
}

#[test]
fn emits_nested_machine_continuations_inline() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state write_line(text: String);
                state exit_process(return_code: i32);
            }

            machine Banner {
                contains console: Console;

                state entry {
                    console.write_line("nested");
                }
            }

            machine main {
                contains banner: Banner;
                contains console: Console;

                state entry {
                    -> banner.entry -> shutdown;
                }

                state shutdown {
                    console.write_line("continued");
                    console.exit_process(0);
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should allow nested continuation");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);
    let schedule = omega_native::state_schedule::build_entry_state_schedule(&native_plan)
        .expect("entry schedule should include nested state");

    assert!(emission_plan.blockers.is_empty());
    assert_eq!(
        schedule
            .iter()
            .map(|state| format!("{}.{}", state.machine, state.state))
            .collect::<Vec<_>>(),
        vec!["main.entry", "Banner.entry", "main.shutdown"]
    );
    assert_eq!(native_plan.host_calls.calls.len(), 3);
    assert_eq!(native_plan.instructions.instructions.len(), 8);
}

#[test]
fn reports_entry_assignments_as_native_mutation_blockers() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state exit_process(return_code: i32);
            }

            machine main {
                contains console: Console;
                owns return_code: i32 = 0;
                owns other_code: i32 = 1;

                state entry {
                    return_code = other_code;
                    console.exit_process(return_code);
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should preserve entry assignment as blocker");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);

    assert!(
        emission_plan.blockers.iter().any(|(_, blocker)| {
            blocker.stage == "state mutation"
                && blocker.reason.contains("return_code")
                && blocker.reason.contains("other_code")
        }),
        "expected entry assignment mutation blocker"
    );
}

#[test]
fn reports_dynamic_text_arguments_as_native_blockers() {
    let tokens = Lexer::new(
        r#"
            data ConsoleLine {
                text: String;
            }

            platform Console {
                state write_line(text: String);
            }

            machine main {
                contains console: Console;
                owns line: ConsoleLine;

                state entry {
                    console.write_line(line.text);
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should preserve dynamic text argument");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);
    let runtime_text = native_plan
        .runtime_text
        .uses
        .iter()
        .find(|(_, text_use)| text_use.expression.display_name() == "line::text")
        .map(|(_, text_use)| text_use)
        .expect("dynamic text argument should be planned");

    assert_eq!(
        runtime_text.source,
        omega_native::runtime_text::RuntimeTextSource::StoredPlace
    );
    assert!(
        emission_plan.blockers.iter().any(|(_, blocker)| {
            blocker.stage == "host arguments"
                && blocker
                    .reason
                    .contains("text argument `line::text` needs runtime string storage lowering")
        }),
        "expected dynamic text argument blocker"
    );
}

#[test]
fn invalidates_static_text_after_mutable_host_output() {
    let tokens = Lexer::new(
        r#"
            data ConsoleLine {
                text: String;
            }

            platform Console {
                state write_line(text: String);
                state read_line(mut out_line: ConsoleLine);
            }

            machine main {
                contains console: Console;
                owns line: ConsoleLine;

                state entry {
                    line.text = "ready";
                    console.write_line(line.text);
                    console.read_line(mut line);
                    console.write_line(line.text);
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should invalidate static text after mutable output");

    assert!(native_plan.runtime_text.uses.iter().any(|(_, text_use)| {
        text_use.statement_index == 3
            && text_use.expression.display_name() == "line::text"
            && text_use.source == omega_native::runtime_text::RuntimeTextSource::StoredPlace
    }));
    assert!(
        native_plan
            .host_calls
            .arguments
            .iter()
            .any(|(_, argument)| matches!(
                &argument.kind,
                omega_native::host_calls::HostCallArgumentKind::Expression(expression)
                    if expression.display_name() == "line::text"
            ))
    );
    assert!(native_plan.data.objects.iter().all(|(_, object)| {
        !(object.source_machine == "main"
            && object.source_state == "entry"
            && object.source_statement == 3)
    }));
}

#[test]
fn plans_state_storage_and_mutations() {
    let tokens = Lexer::new(
        r#"
            data Room {
                label: String;
            }

            machine main {
                owns current_room: Room;

                state entry {
                    let scratch: Room;
                    scratch = current_room;
                    current_room.label = "A1";
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should collect state storage");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);

    assert_eq!(native_plan.state_storage.locals.len(), 1);
    assert_eq!(native_plan.state_storage.mutations.len(), 2);
    assert!(
        native_plan
            .runtime_storage
            .frame_slots
            .iter()
            .any(|(_, slot)| {
                slot.name == "scratch"
                    && slot.type_name == "Room"
                    && slot.byte_offset == 0
                    && slot.byte_size == 16
                    && slot.alignment == 8
            })
    );
    assert!(native_plan.runtime_storage.writes.iter().any(|(_, write)| {
        write.target.display_name() == "scratch"
            && write.lowering == omega_native::state_storage::StateMutationLowering::NeedsLocalWrite
    }));
    assert!(
        native_plan
            .state_storage
            .mutations
            .iter()
            .any(|(_, mutation)| mutation.mutation_kind
                == omega_native::state_storage::StateMutationKind::MachineOwned)
    );
    assert!(
        native_plan
            .state_storage
            .mutations
            .iter()
            .any(|(_, mutation)| mutation.lowering
                == omega_native::state_storage::StateMutationLowering::AlreadyLowered)
    );
    assert!(
        native_plan
            .state_storage
            .mutations
            .iter()
            .any(|(_, mutation)| mutation.lowering
                == omega_native::state_storage::StateMutationLowering::NeedsLocalWrite)
    );
    assert!(
        emission_plan
            .blockers
            .iter()
            .any(|(_, blocker)| blocker.stage == "state storage")
    );
    assert!(
        emission_plan
            .blockers
            .iter()
            .any(|(_, blocker)| blocker.stage == "state mutation")
    );
}

#[test]
fn plans_required_state_value_uses() {
    let tokens = Lexer::new(
        r#"
            data Line {
                text: String;
            }

            machine main {
                owns line: Line;

                state entry {
                    line.text = "Room " + "A1";
                    -> done when line.text == "Room A1";
                }

                state done {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should collect value uses");

    assert!(native_plan.state_values.values.iter().any(|(_, value)| {
        value.required
            && value.kind == omega_native::state_values::StateValueKind::Binary
            && value.role == omega_native::state_values::StateValueRole::TransitionGuard
    }));
    assert!(
        native_plan
            .runtime_text
            .writes
            .iter()
            .any(|(_, text_write)| {
                text_write.target.display_name() == "line::text"
                    && text_write.kind
                        == omega_native::runtime_text::RuntimeTextWriteKind::GeneratedString
            })
    );
    let (_, builder) = native_plan
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| builder.target.display_name() == "line::text")
        .expect("generated text write should have a string builder plan");
    let segments = native_plan
        .runtime_text
        .builder_segments
        .span(builder.segments)
        .expect("builder segments should resolve");

    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].expression.display_name(), "\"Room \"");
    assert_eq!(
        segments[0].kind,
        omega_native::runtime_text::RuntimeTextBuilderSegmentKind::StaticText
    );
    assert_eq!(segments[1].expression.display_name(), "\"A1\"");
    assert_eq!(
        segments[1].kind,
        omega_native::runtime_text::RuntimeTextBuilderSegmentKind::StaticText
    );
}

#[test]
fn skips_state_value_blocker_for_planned_runtime_text_builder() {
    let tokens = Lexer::new(
        r#"
            data Line {
                text: String;
            }

            platform Console {
                state write_line(text: String);
            }

            machine main {
                contains console: Console;
                owns line: Line;

                state entry {
                    line.text = "Room " + "A1";
                    console.write_line(line.text);

                    -> self;
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    crate::semantic::validation::validate_program(&program).expect("validation should pass");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should build runtime text builder");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);

    assert!(
        native_plan
            .runtime_text
            .builders
            .iter()
            .any(|(_, builder)| builder.target.display_name() == "line::text")
    );
    assert!(
        !emission_plan
            .blockers
            .iter()
            .any(|(_, blocker)| blocker.stage == "state values"),
        "planned text builders should not report a duplicate state-value blocker"
    );
    assert!(
        !emission_plan
            .blockers
            .iter()
            .any(|(_, blocker)| blocker.stage == "state mutation"),
        "planned text write should not report a stale state-mutation blocker"
    );
}

#[test]
fn lowers_constant_integer_assignment_before_host_call() {
    let tokens = Lexer::new(
        r#"
            platform Console {
                state exit_process(return_code: i32);
            }

            machine main {
                contains console: Console;
                owns return_code: i32 = 1;

                state entry {
                    return_code = 0;
                    console.exit_process(return_code);
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should track constant integer assignment");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);
    let exit_call = native_plan
        .host_calls
        .calls
        .iter()
        .find(|(_, call)| call.platform_call == "console.exit_process")
        .map(|(_, call)| call)
        .expect("exit call should be lowered");
    let arguments = native_plan
        .host_calls
        .arguments
        .span(exit_call.arguments)
        .expect("exit arguments should be present");

    assert!(emission_plan.blockers.is_empty());
    assert_eq!(
        arguments[0].kind,
        omega_native::host_calls::HostCallArgumentKind::Integer(0)
    );
}

#[test]
fn selects_static_guarded_transition() {
    let tokens = Lexer::new(
        r#"
            data CommandKind {
                Quit,
                Look,
                Invalid,
            }

            data Command {
                kind: CommandKind;
            }

            platform Console {
                state write_line(text: String);
                state exit_process(return_code: i32);
            }

            machine main {
                contains console: Console;
                owns command: Command;

                state entry {
                    command.kind = CommandKind::Look;

                    -> look when command.kind == CommandKind::Look;
                    -> quit when command.kind == CommandKind::Quit;
                    -> invalid;
                }

                state look {
                    console.write_line("look");
                    console.exit_process(0);
                }

                state quit {
                    console.write_line("quit");
                    console.exit_process(0);
                }

                state invalid {
                    console.write_line("invalid");
                    console.exit_process(1);
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should select a static guarded transition");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);
    let schedule = omega_native::state_schedule::build_entry_state_schedule(&native_plan)
        .expect("entry schedule should select look branch");

    assert!(emission_plan.blockers.is_empty());
    assert_eq!(
        schedule
            .iter()
            .map(|state| format!("{}.{}", state.machine, state.state))
            .collect::<Vec<_>>(),
        vec!["main.entry", "main.look"]
    );
    assert_eq!(native_plan.host_calls.calls.len(), 6);
    assert_eq!(native_plan.instructions.instructions.len(), 6);
}

#[test]
fn propagates_static_state_call_arguments() {
    let tokens = Lexer::new(
        r#"
            data CellId {
                Empty,
                A1,
            }

            data Exit {
                destination: CellId;
            }

            data Room {
                exits: [Exit; 2];
            }

            machine main {
                owns room: Room;

                state entry {
                    build_room(mut room);
                    append_exit(room.exits[0]);
                    append_exit(room.exits[1]);
                }

                state build_room(mut out_room: Room) {
                    out_room.exits[0] = Exit { destination: CellId::A1 };
                    out_room.exits[1] = Exit { destination: CellId::Empty };
                }

                state append_exit(exit: Exit) {
                    -> append_open_exit when exit.destination != CellId::Empty;
                    ->
                }

                state append_open_exit {
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should propagate static state arguments");
    let schedule = omega_native::state_schedule::build_entry_state_schedule(&native_plan)
        .expect("entry schedule should evaluate helper-state guards");
    let scheduled_states = schedule
        .iter()
        .map(|state| format!("{}.{}", state.machine, state.state))
        .collect::<Vec<_>>();

    assert_eq!(
        scheduled_states
            .iter()
            .filter(|state| state.as_str() == "main.append_open_exit")
            .count(),
        1
    );
}

#[test]
fn lowers_mutable_output_host_call() {
    let tokens = Lexer::new(
        r#"
            data ConsoleLine {
                text: String;
            }

            platform Console {
                state write(text: String);
                state write_line(text: String);
                state read_line(mut out_line: ConsoleLine);
                state exit_process(return_code: i32);
            }

            machine main {
                contains console: Console;
                owns line: ConsoleLine;

                state entry {
                    console.write("> ");
                    console.read_line(mut line);
                    console.write_line(line.text);
                    console.exit_process(0);
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should lower mutable output host call");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);
    let read_buffer = native_plan
        .data
        .objects
        .iter()
        .find(|(_, object)| object.source_statement == 1)
        .map(|(_, object)| object)
        .expect("read_line should allocate a mutable output buffer");
    let read_buffer_bytes = native_plan
        .data
        .bytes
        .span(read_buffer.bytes)
        .expect("read buffer bytes should be present");
    let runtime_text_buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| buffer.target.display_name() == "line")
        .map(|(_, buffer)| buffer)
        .expect("read_line should plan a runtime text buffer binding");
    let runtime_text_slot = native_plan
        .runtime_text
        .slots
        .iter()
        .find(|(_, slot)| slot.place.display_name() == "line::text")
        .map(|(_, slot)| slot)
        .expect("read_line should plan a runtime text slot");

    assert!(emission_plan.blockers.is_empty());
    assert_eq!(native_plan.host_calls.calls.len(), 4);
    assert!(
        native_plan
            .instructions
            .operands
            .iter()
            .any(|(_, operand)| matches!(
                &operand.kind,
                omega_native::instructions::InstructionOperandKind::DataAddress { symbol }
                    if symbol == &read_buffer.symbol
            )),
        "stdout should be able to reuse the input buffer as a runtime text operand"
    );
    assert_eq!(read_buffer_bytes.len(), 256);
    assert_eq!(runtime_text_buffer.byte_capacity, 256);
    assert_eq!(runtime_text_slot.byte_capacity, 256);
    assert!(runtime_text_slot.has_input_buffer);
}

#[test]
fn lowers_static_record_array_field_text() {
    let tokens = Lexer::new(
        r#"
            data CellId {
                Empty,
                A1,
                A2,
            }

            data Exit {
                command: String;
                destination: CellId;
            }

            data Room {
                label: String;
                exits: [Exit; 2];
            }

            platform Console {
                state write_line(text: String);
                state exit_process(return_code: i32);
            }

            machine main {
                contains console: Console;
                owns room: Room;
                owns selected_exit: Exit;

                state entry {
                    room.label = "A1";
                    room.exits[0] = Exit { command: "north", destination: CellId::A2 };
                    selected_exit = room.exits[0];

                    console.write_line(room.label);
                    console.write_line(selected_exit.command);
                    console.exit_process(0);
                }
            }
            "#,
    )
    .tokenize()
    .expect("tokenization should succeed");
    let parsed = parse_file(&tokens).expect("parse should succeed");
    let program = lower_program(&parsed.items).expect("lowering should succeed");
    let native_plan = omega_native::plan::build_native_plan(
        &program,
        omega_native::target::NativeTarget::macos_arm64(),
    )
    .expect("native planning should lower static record field text");
    let emission_plan = omega_native::emission::build_emission_plan(&native_plan);

    assert!(emission_plan.blockers.is_empty());
    assert_eq!(native_plan.host_calls.calls.len(), 3);
    assert_eq!(native_plan.data.objects.len(), 4);
    assert!(native_plan.data.objects.iter().any(|(_, data_object)| {
        native_plan
            .data
            .bytes
            .span(data_object.bytes)
            .is_some_and(|bytes| bytes == b"A1")
    }));
    assert!(native_plan.data.objects.iter().any(|(_, data_object)| {
        native_plan
            .data
            .bytes
            .span(data_object.bytes)
            .is_some_and(|bytes| bytes == b"north")
    }));
}

#[test]
fn selected_target_loads_only_referenced_host_package() {
    let build_dir = std::env::temp_dir().join(format!(
        "omega-compiler-selected-target-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&build_dir);
    let root_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../samples/cli_mvp/main.omg");

    let output = crate::check(crate::CompileOptions {
        build_dir: Some(build_dir.clone()),
        root_path,
        target_name: Some("windows_x64".to_owned()),
    })
    .expect("check should pass");
    let sources = std::fs::read_to_string(output.artifacts_dir.join("01_sources.txt"))
        .expect("source artifact should be readable");

    assert!(sources.contains("omega/host/targets/windows/mod.omg"));
    assert!(sources.contains("omega/host/targets/windows/kernel32.omg"));
    assert!(sources.contains("omega/host/contracts/mod.omg"));
    assert!(!sources.contains("omega/host/targets/linux/mod.omg"));
    assert!(!sources.contains("omega/host/targets/darwin/mod.omg"));

    let trust = std::fs::read_to_string(output.artifacts_dir.join("10_trust.txt"))
        .expect("trust artifact should be readable");
    assert!(trust.contains("targets: 1"));
    assert!(trust.contains("trust roots: 7"));
    assert!(trust.contains("unresolved trusts: 0"));
    assert!(trust.contains("target `windows_x64`"));
    assert!(trust.contains("trust `omega_windows_kernel32`"));
    assert!(!trust.contains("target `linux_x64`"));
    assert!(!trust.contains("unchecked `invariant_proofs`"));

    let _ = std::fs::remove_dir_all(build_dir);
}

#[test]
fn checks_every_sample_entrypoint() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler");
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
        .nth(3)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler");
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
        .nth(3)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler");
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
        let entry = entry.unwrap_or_else(|error| panic!("failed to read directory entry: {error}"));
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
