use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalExecutionResult, TerminalInterpretError,
    TerminalScalarValue, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

#[path = "scalar_return_calls_source/call_result_bounds.rs"]
mod call_result_bounds;
#[path = "scalar_return_calls_source/integer_computations.rs"]
mod integer_computations;

fn encoded(source: &str) -> (Vec<u8>, Vec<u8>) {
    encoded_arms(source, false)
}

fn encoded_arms(source: &str, combined: bool) -> (Vec<u8>, Vec<u8>) {
    let checked = checked_arms(source, combined);
    let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}, combined={combined}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).expect("encode semantics"),
        encode_proof_bundle(&lowered.proof_bundle).expect("encode proof"),
    )
}

fn checked_arms(source: &str, combined: bool) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse");
    if combined {
        combine_value_machine_arms(&mut syntax);
    }
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn combine_value_machine_arms(syntax: &mut syntax_trees::SyntaxTrees) {
    use syntax_trees::statement::StatementNode;

    let machine = syntax
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) if machine.name.as_str() == "value" => {
                Some(machine.clone())
            }
            _ => None,
        })
        .expect("value machine");
    let state = syntax.items.state_handles(machine.states)[0];
    let statements = syntax.items.state(state).statements;
    let handles = syntax.items.statements(statements);
    let [first, second] = &handles[handles.len() - 2..] else {
        panic!("two terminal arms");
    };
    let first = *first;
    let second = *second;
    let StatementNode::Transition(mut transition) = syntax.statements.statement(first).clone()
    else {
        panic!("first arm");
    };
    let StatementNode::Transition(continuation) = syntax.statements.statement(second) else {
        panic!("second arm");
    };
    assert!(!transition.continuation.is_valid());
    assert!(!continuation.continuation.is_valid());
    // The authored true/false arms are exhaustive. Exercise the equivalent
    // combined representation before resolution chooses arm-local captures.
    transition.continuation = continuation.target;
    syntax
        .statements
        .replace_statement(first, StatementNode::Transition(transition));
    syntax.items.state_mut(state).statements =
        arena::HandleSpan::from_parts(statements.start(), statements.count() - 1);
}

fn execute(
    artifact: &(Vec<u8>, Vec<u8>),
    arguments: &[TerminalScalarValue],
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    // The producer has returned; only bytes and fresh runtime inputs cross
    // the interpreter's independent decoding and proof-verification boundary.
    interpret_terminal_artifact(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        arguments,
    )
}

#[test]
fn guarded_parenthesized_state_calls_remain_transitions() {
    let source = r#"
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            transition flag {
                true -> (finish(flag && true))
                false -> false
            }
            state finish(input: bool) -> bool { input }
        }
    "#;
    for combined in [false, true] {
        let checked = checked_arms(source, combined);
        let value = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .expect("value machine");
        assert_eq!(checked.typed.machine_states(value).len(), 2);
        let artifact = encoded_arms(source, combined);
        for flag in [false, true] {
            let input = TerminalScalarValue::Boolean(flag);
            assert_eq!(
                execute(&artifact, &[input]).unwrap(),
                TerminalExecutionResult::Scalar(input)
            );
        }
    }
}

#[test]
fn direct_boolean_return_call_executes_for_both_inputs() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }

        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        { identity(flag) }
    "#;
    let artifact = encoded(source);
    for flag in [false, true] {
        let value = TerminalScalarValue::Boolean(flag);
        assert_eq!(
            execute(&artifact, &[value]).unwrap(),
            TerminalExecutionResult::Scalar(value)
        );
    }
}

#[test]
fn direct_integer_return_call_preserves_runtime_values() {
    let source = r#"
        machine identity(input: i32) -> i32
        requires 7i32 == 7i32
        ensures 7i32 == 7i32
        { input }

        machine value(input: i32) -> i32
        requires 7i32 == 7i32
        ensures 7i32 == 7i32
        { identity(input) }
    "#;
    let artifact = encoded(source);
    for input in [-128, 0, 7, 300] {
        let value = TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            value: IntegerValue::Signed(input),
        };
        assert_eq!(
            execute(&artifact, &[value]).unwrap(),
            TerminalExecutionResult::Scalar(value)
        );
    }
}

#[test]
fn return_call_arguments_distinguish_current_storage_and_saved_values() {
    for (arguments, expected) in [("current, saved", true), ("saved, current", false)] {
        let source = format!(
            r#"
            machine compare(first: bool, second: bool) -> bool
            requires true == true
            ensures true == true
            {{ first && !second }}

            machine value() -> bool
            requires {expected} == {expected}
            ensures {expected} == {expected}
            {{
                let mut current: bool = false;
                let saved: bool = current;
                current = true;
                compare({arguments})
            }}
        "#
        );
        assert_eq!(
            execute(&encoded(&source), &[]).unwrap(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
        );
    }
}

#[test]
fn unconditional_transition_value_call_executes_after_serialization() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }

        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        { transition { _ -> (identity(flag)) } }
    "#;
    let artifact = encoded(source);
    for flag in [false, true] {
        let value = TerminalScalarValue::Boolean(flag);
        assert_eq!(
            execute(&artifact, &[value]).unwrap(),
            TerminalExecutionResult::Scalar(value)
        );
    }
}

#[test]
fn return_call_preserves_normal_and_explicit_crash_outcomes() {
    for (cause_name, cause) in [
        ("Trap", terminal_psi::CrashCause::Trap),
        ("Abort", terminal_psi::CrashCause::Abort),
    ] {
        let source = format!(
            r#"
            machine maybe_crash(flag: bool) -> i32
            requires 7i32 == 7i32
            ensures 7i32 == 7i32
            crashes {cause_name}
                flag
            {{
                transition {{ flag -> fail() _ -> succeed() }}
                state fail() -> i32 {{ crash {cause_name}; }}
                state succeed() -> i32 {{ 7i32 }}
            }}

            machine value(flag: bool) -> i32
            requires 7i32 == 7i32
            ensures 7i32 == 7i32
            crashes {cause_name}
                flag
            {{ maybe_crash(flag) }}
        "#
        );
        let artifact = encoded(&source);
        assert_eq!(
            execute(&artifact, &[TerminalScalarValue::Boolean(false)]).unwrap(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                value: IntegerValue::Signed(7),
            }),
        );
        let error = execute(&artifact, &[TerminalScalarValue::Boolean(true)]).unwrap_err();
        let TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)) = error
        else {
            panic!("expected {cause_name}, got {error:#?}");
        };
        assert_eq!(crash.cause, cause);
        assert!(crash.frontier_lower_bound.is_empty());
    }
}

#[test]
fn guarded_parameter_name_return_call_executes_in_both_forms() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }

        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        { transition flag { true -> (identity(flag)) false -> false } }
    "#;
    for combined in [false, true] {
        let artifact = encoded_arms(source, combined);
        for flag in [false, true] {
            let value = TerminalScalarValue::Boolean(flag);
            assert_eq!(
                execute(&artifact, &[value]).unwrap(),
                TerminalExecutionResult::Scalar(value)
            );
        }
    }
}

fn assert_guarded_computed_return_arguments(combined: bool) {
    let source = r#"
        machine compare(first: bool, second: bool) -> bool
        requires true == true
        ensures true == true
        { first && !second }

        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            let mut current: bool = false;
            let saved: bool = current;
            current = true;
            transition flag {
                true -> (compare(current && !saved, saved && current))
                false -> (compare(saved || current, current && !saved))
            }
        }
    "#;
    let artifact = encoded_arms(source, combined);
    for flag in [false, true] {
        let value = TerminalScalarValue::Boolean(flag);
        assert_eq!(
            execute(&artifact, &[value]).unwrap(),
            TerminalExecutionResult::Scalar(value)
        );
    }
}

#[test]
fn separate_guarded_calls_materialize_computed_current_and_saved_arguments() {
    assert_guarded_computed_return_arguments(false);
}

#[test]
fn combined_guarded_calls_materialize_computed_current_and_saved_arguments() {
    assert_guarded_computed_return_arguments(true);
}

#[test]
fn guarded_calls_keep_literal_arguments_and_partial_integer_arguments_selected() {
    let source = r#"
        machine identity(input: u8) -> u8
        requires 7u8 == 7u8
        ensures 7u8 == 7u8
        { input }

        machine value(denominator: u8) -> u8
        requires 7u8 == 7u8
        ensures 7u8 == 7u8
        {
            transition denominator == 0 {
                true -> (identity(7u8))
                false -> (identity(7u8 / denominator))
            }
        }
    "#;
    for combined in [false, true] {
        let artifact = encoded_arms(source, combined);
        for (denominator, expected) in [(0, 7), (1, 7), (2, 3), (7, 1), (255, 0)] {
            let integer = |value| TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: IntegerValue::Unsigned(value),
            };
            assert_eq!(
                execute(&artifact, &[integer(denominator)]).unwrap(),
                TerminalExecutionResult::Scalar(integer(expected))
            );
        }
    }
}

fn assert_guarded_crash_call_is_selective(combined: bool) {
    for (cause_name, cause) in [
        ("Trap", terminal_psi::CrashCause::Trap),
        ("Abort", terminal_psi::CrashCause::Abort),
    ] {
        for call_when in [false, true] {
            let call = "(maybe_crash(current && !saved))";
            let (when_true, when_false) = if call_when {
                (call, "(false)")
            } else {
                ("(false)", call)
            };
            let source = format!(
                r#"
                machine maybe_crash(input: bool) -> bool
                requires false == false
                ensures false == false
                crashes {cause_name}
                    input
                {{
                    transition input {{ true -> fail() false -> (false) }}
                    state fail() -> bool {{ crash {cause_name}; }}
                }}

                machine value(flag: bool) -> bool
                requires false == false
                ensures false == false
                crashes {cause_name}
                {{
                    let mut current: bool = false;
                    let saved: bool = current;
                    current = true;
                    transition flag {{ true -> {when_true} false -> {when_false} }}
                }}
            "#
            );
            let artifact = encoded_arms(&source, combined);
            assert_eq!(
                execute(&artifact, &[TerminalScalarValue::Boolean(!call_when)]).unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false)),
                "the unselected crashing call must not execute",
            );
            let error = execute(&artifact, &[TerminalScalarValue::Boolean(call_when)]).unwrap_err();
            let TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)) =
                error
            else {
                panic!("expected selected {cause_name}, got {error:#?}");
            };
            assert_eq!(crash.cause, cause);
            assert!(crash.frontier_lower_bound.is_empty());
        }
    }
}

#[test]
fn separate_guarded_calls_do_not_execute_unselected_trap_or_abort() {
    assert_guarded_crash_call_is_selective(false);
}

#[test]
fn combined_guarded_calls_do_not_execute_unselected_trap_or_abort() {
    assert_guarded_crash_call_is_selective(true);
}

#[test]
fn nested_scalar_argument_calls_preserve_snapshots_and_boolean_composition() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine negate(input: bool) -> bool
        requires true == true
        ensures true == true
        { !input }
        machine compare(first: bool, second: bool) -> bool
        requires true == true
        ensures true == true
        { first == second }
        machine value(selected: bool, flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            let mut current: bool = flag;
            let saved: bool = current;
            current = !flag;
            transition selected {
                true -> finish(saved, flag && compare(identity(current), negate(saved)))
                false -> finish(saved, flag || !(identity(current) != negate(saved)))
            }
            state finish(first: bool, second: bool) -> bool { first != second }
        }
    "#;
    for combined in [false, true] {
        let artifact = encoded_arms(source, combined);
        for selected in [false, true] {
            for flag in [false, true] {
                assert_eq!(
                    execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            TerminalScalarValue::Boolean(flag)
                        ]
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                        !selected && !flag
                    )),
                    "selected={selected}, flag={flag}, combined={combined}",
                );
            }
        }
    }
}

#[test]
fn outer_arm_selection_keeps_redundant_inner_calls_unexecuted() {
    let source = r#"
        machine effect() -> bool
        crashes Abort
        { crash Abort; }
        machine value(flag: bool) -> bool
        requires false == false
        ensures false == false
        crashes Abort
        {
            transition flag {
                true -> finish(flag || effect())
                false -> finish(flag && effect())
            }
            state finish(input: bool) -> bool { input }
        }
    "#;
    for combined in [false, true] {
        let artifact = encoded_arms(source, combined);
        for flag in [false, true] {
            let input = TerminalScalarValue::Boolean(flag);
            assert_eq!(
                execute(&artifact, &[input]).unwrap(),
                TerminalExecutionResult::Scalar(input)
            );
        }
    }
}

#[test]
fn computed_call_results_bind_guarded_callee_crash_routes() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires false == false
        ensures false == false
        { input }
        machine maybe_crash(input: bool) -> bool
        requires false == false
        ensures false == false
        crashes Abort
            input
        {
            transition input { true -> fail() false -> false }
            state fail() -> bool { crash Abort; }
        }
        machine value(selected: bool, flag: bool) -> bool
        requires false == false
        ensures false == false
        crashes Abort
        {
            transition selected {
                true -> finish(flag || maybe_crash(identity(!flag)))
                false -> finish(false)
            }
            state finish(input: bool) -> bool { input }
        }
    "#;
    for combined in [false, true] {
        let artifact = encoded_arms(source, combined);
        for selected in [false, true] {
            for flag in [false, true] {
                let result = execute(
                    &artifact,
                    &[
                        TerminalScalarValue::Boolean(selected),
                        TerminalScalarValue::Boolean(flag),
                    ],
                );
                if selected && !flag {
                    let TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(
                        crash,
                    )) = result.unwrap_err()
                    else {
                        panic!("selected computed true argument must abort");
                    };
                    assert_eq!(crash.cause, terminal_psi::CrashCause::Abort);
                } else {
                    assert_eq!(
                        result.unwrap(),
                        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                            selected && flag
                        ))
                    );
                }
            }
        }
    }
}

#[test]
fn scalar_computation_custody_mutations_reject_before_publication() {
    use checked_trees::CheckedScalarComputationKind;
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine value(selected: bool, flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            transition selected {
                true -> finish(flag && identity(flag))
                false -> finish(false)
            }
            state finish(input: bool) -> bool { input }
        }
    "#;
    for combined in [false, true] {
        let checked = checked_arms(source, combined);
        checked_trees_to_terminal_psi::lower_machine(&checked, "value").unwrap();
        let plans = &checked.facts.values.scalar_computations;
        let (root_handle, root) = plans.roots.iter().next().expect("computation root");
        let root = root.clone();
        let (call_handle, _) = plans
            .nodes
            .iter()
            .find(|(_, node)| matches!(node.kind, CheckedScalarComputationKind::Call { .. }))
            .expect("nested call");
        for mutation in 0..10 {
            let mut mutated = checked.clone();
            let plans = &mut mutated.facts.values.scalar_computations;
            match mutation {
                0 => {
                    plans.roots.append(root.clone());
                }
                1 => {
                    plans.roots.get_mut(root_handle).root = arena::Handle::invalid();
                }
                2 => {
                    plans.nodes.get_mut(root.root).primitive_type =
                        typed_trees::types::PrimitiveType::I32;
                }
                3..=5 => {
                    let CheckedScalarComputationKind::Call {
                        source_call,
                        call_ordinal,
                        arguments,
                        ..
                    } = &mut plans.nodes.get_mut(call_handle).kind
                    else {
                        unreachable!()
                    };
                    match mutation {
                        3 => *source_call = arena::Handle::invalid(),
                        4 => *call_ordinal += 1,
                        5 => *arguments = arena::HandleSpan::empty(),
                        _ => unreachable!(),
                    }
                }
                6 => {
                    let CheckedScalarComputationKind::Select { condition, .. } =
                        &mut plans.nodes.get_mut(root.root).kind
                    else {
                        panic!("conditional root")
                    };
                    *condition = root.root;
                }
                7 => {
                    let CheckedScalarComputationKind::Call { target_state, .. } =
                        &mut plans.nodes.get_mut(call_handle).kind
                    else {
                        unreachable!()
                    };
                    *target_state = symbols::SymbolHandle::invalid();
                }
                8 => {
                    let CheckedScalarComputationKind::Select {
                        when_true,
                        when_false,
                        ..
                    } = &mut plans.nodes.get_mut(root.root).kind
                    else {
                        unreachable!()
                    };
                    *when_false = *when_true;
                }
                9 => {
                    plans.nodes.get_mut(call_handle).primitive_type =
                        typed_trees::types::PrimitiveType::I32;
                }
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_terminal_psi::lower_machine(&mutated, "value").is_err(),
                "mutation={mutation}, combined={combined}"
            );
        }
    }
}

#[test]
fn guarded_state_argument_short_circuit_executes_only_selected_calls() {
    let source = r#"
        machine effect() -> bool
        crashes Abort
        { crash Abort; }

        machine value(selected: bool, flag: bool) -> bool
        requires false == false
        ensures false == false
        crashes Abort
        {
            transition selected {
                true -> finish(false && effect())
                false -> finish(false)
            }
            state finish(result: bool) -> bool { result }
        }
    "#;
    // The typed computation graph must retain both the outer arm selection
    // and the independent short-circuit selection inside its argument.
    for (operand, when_false, when_true) in [
        ("false && effect()", Some(false), Some(false)),
        ("true || effect()", Some(true), Some(true)),
        ("flag && effect()", Some(false), None),
        ("flag || effect()", None, Some(true)),
    ] {
        let source = source.replace("false && effect()", operand);
        for combined in [false, true] {
            let artifact = encoded_arms(&source, combined);
            for selected in [false, true] {
                for flag in [false, true] {
                    let expected = if !selected {
                        Some(false)
                    } else if flag {
                        when_true
                    } else {
                        when_false
                    };
                    let result = execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            TerminalScalarValue::Boolean(flag),
                        ],
                    );
                    if let Some(expected) = expected {
                        assert_eq!(
                            result.unwrap(),
                            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
                            "{operand}, selected={selected}, flag={flag}, combined={combined}"
                        );
                    } else {
                        let TerminalArtifactInterpretError::Execution(
                            TerminalInterpretError::Crash(crash),
                        ) = result.unwrap_err()
                        else {
                            panic!("selected RHS must abort");
                        };
                        assert_eq!(crash.cause, terminal_psi::CrashCause::Abort);
                        assert!(crash.frontier_lower_bound.is_empty());
                    }
                }
            }
        }
    }
}
