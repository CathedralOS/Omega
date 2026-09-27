use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalExecutionResult, TerminalInterpretError,
    TerminalScalarValue, interpret_terminal_artifact,
};

#[path = "scalar_return_calls_source/call_result_bounds.rs"]
mod call_result_bounds;
#[path = "scalar_return_calls_source/computed_assignments.rs"]
mod computed_assignments;
#[path = "scalar_return_calls_source/computed_guards.rs"]
mod computed_guards;
#[path = "scalar_return_calls_source/computed_initializers.rs"]
mod computed_initializers;
#[path = "scalar_return_calls_source/computed_returns.rs"]
mod computed_returns;
#[path = "scalar_return_calls_source/crash_destinations.rs"]
mod crash_destinations;
#[path = "scalar_return_calls_source/integer_computations.rs"]
mod integer_computations;
#[path = "scalar_return_calls_source/parameter_result_contracts.rs"]
mod parameter_result_contracts;
#[path = "scalar_return_calls_source/pure_source_custody.rs"]
mod pure_source_custody;

fn encoded(source: &str) -> (Vec<u8>, Vec<u8>) {
    encoded_arms(source, false)
}

fn encoded_arms(source: &str, combined: bool) -> (Vec<u8>, Vec<u8>) {
    let checked = checked_arms(source, combined);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("value"),
    )
    .unwrap_or_else(|error| panic!("{source}, combined={combined}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).expect("encode semantics"),
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof"),
    )
}

fn checked_arms(source: &str, combined: bool) -> checked_trees::CheckedTrees {
    crate::front_end::checked_program_with_syntax_edit(source, |syntax| {
        if combined {
            combine_value_machine_arms(syntax);
        }
    })
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
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("value"),
        )
        .unwrap();
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
                checked_trees_to_lowered_psi::lower_machine(
                    &mutated,
                    TerminalMachineSelection::Name("value")
                )
                .is_err(),
                "mutation={mutation}, combined={combined}"
            );
        }
    }
}
