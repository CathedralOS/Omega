use super::*;
use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpressionRole};
use typed_trees::statement::{StatementNode, TransitionGuardNode};

fn dispatch(predicate: &str, when_true: &str, when_false: &str, form: usize) -> String {
    match form {
        0 => format!("transition {{ {predicate} -> {when_true}\n _ -> {when_false} }}"),
        1 => format!("transition {predicate} {{ true -> {when_true}\n false -> {when_false} }}"),
        2 => format!("transition {predicate} {{ false -> {when_false}\n true -> {when_true} }}"),
        _ => unreachable!(),
    }
}

fn assert_guard_roots(checked: &checked_trees::CheckedTrees, names: &[&str], state_count: usize) {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let states = checked.typed.machine_states(machine);
    assert_eq!(states.len(), state_count, "no source continuation states");
    let mut local_names = Vec::new();
    let mut guards = 0;
    for state in states {
        for (ordinal, statement) in checked
            .typed
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            match statement {
                StatementNode::LocalData(local) => local_names.push(local.name.as_str()),
                StatementNode::Transition(transition) => {
                    if let TransitionGuardNode::When(expression) = transition.guard {
                        guards += 1;
                        let roots: Vec<_> = checked
                            .facts
                            .values
                            .scalar_computations
                            .roots
                            .iter()
                            .filter(|(_, root)| {
                                root.machine == machine.symbol
                                    && root.state == state.symbol
                                    && root.statement_ordinal == ordinal as u32
                                    && root.role == CheckedScalarExpressionRole::Guard
                            })
                            .collect();
                        assert_eq!(roots.len(), 1, "one authored guard root at {ordinal}");
                        let node = checked
                            .facts
                            .values
                            .scalar_computations
                            .nodes
                            .get(roots[0].1.root);
                        assert_eq!(
                            node.authored_root, expression,
                            "root belongs to exact source When"
                        );
                        assert_eq!(node.primitive_type, typed_trees::types::PrimitiveType::Bool);
                    }
                }
                _ => {}
            }
        }
    }
    assert_eq!(local_names, names, "no hoisted guard temporaries");
    assert_eq!(guards, 1, "the final fallback is unconditional");
}

fn encoded_guards(source: &str, names: &[&str], states: usize) -> (Vec<u8>, Vec<u8>) {
    encoded_guard_arms(source, names, states, false)
}

fn encoded_guard_arms(
    source: &str,
    names: &[&str],
    states: usize,
    combined: bool,
) -> (Vec<u8>, Vec<u8>) {
    let checked = checked_arms(source, combined);
    assert_guard_roots(&checked, names, states);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_bundle(&lowered.proof_bundle).unwrap(),
    )
}

fn unsigned(bits: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn computed_guard_comparisons_preserve_both_ordinary_arm_orders_and_anonymous_dispatch() {
    for (operator, expected) in [
        ("==", false),
        ("!=", true),
        ("<", true),
        ("<=", true),
        (">", false),
        (">=", false),
    ] {
        for form in 0..3 {
            let predicate = format!("identity(left) {operator} identity(right)");
            let dispatch = dispatch(&predicate, "true", "false", form);
            let source = format!(
                r#"
                machine identity(input: u8) -> u8
                requires 0u8 == 0u8
                ensures 0u8 == 0u8
                {{ input }}
                machine value(left: u8, right: u8) -> bool
                requires true == true
                ensures true == true
                {{ {dispatch} }}
            "#
            );
            let artifact = encoded_guards(&source, &[], 1);
            assert_eq!(
                execute(&artifact, &[unsigned(8, 3), unsigned(8, 9)]).unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
                "operator={operator}, form={form}"
            );
            assert_eq!(
                execute(&artifact, &[unsigned(8, 9), unsigned(8, 9)]).unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(matches!(
                    operator,
                    "==" | "<=" | ">="
                )))
            );
        }
    }
}

#[test]
fn computed_guards_skip_unselected_crashing_boolean_operands() {
    for (operator, skipped) in [("&&", false), ("||", true)] {
        for (cause, expected) in [
            ("Abort", terminal_psi::CrashCause::Abort),
            ("Trap", terminal_psi::CrashCause::Trap),
        ] {
            for form in 0..3 {
                let dispatch =
                    dispatch(&format!("flag {operator} effect()"), "true", "false", form);
                let source = format!(
                    r#"
                    machine effect() -> bool crashes {cause} {{ crash {cause}; }}
                    machine value(flag: bool) -> bool
                    requires true == true
                    ensures true == true
                    crashes {cause}
                    {{ {dispatch} }}
                "#
                );
                let artifact = encoded_guards(&source, &[], 1);
                assert_eq!(
                    execute(&artifact, &[TerminalScalarValue::Boolean(skipped)]).unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(skipped))
                );
                assert!(
                    matches!(execute(&artifact, &[TerminalScalarValue::Boolean(!skipped)]),
                    Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == expected)
                );
            }
        }
    }
}

#[test]
fn computed_guard_calls_precede_cast_wrapped_later_operands_and_target_calls() {
    for (first, second, expected) in [
        ("Abort", "Trap", terminal_psi::CrashCause::Abort),
        ("Trap", "Abort", terminal_psi::CrashCause::Trap),
    ] {
        for form in 0..3 {
            let dispatch = dispatch(
                "first() > (second() as u16)",
                "(target())",
                "(target())",
                form,
            );
            let source = format!(
                r#"
                machine first() -> u16 crashes {first} {{ crash {first}; }}
                machine second() -> u8 crashes {second} {{ crash {second}; }}
                machine target() -> bool crashes {second} {{ crash {second}; }}
                machine value() -> bool
                requires true == true
                ensures true == true
                crashes Abort
                crashes Trap
                {{ {dispatch} }}
            "#
            );
            let artifact = encoded_guards(&source, &[], 1);
            assert!(
                matches!(execute(&artifact, &[]),
                Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == expected),
                "form={form}, first={first}"
            );
        }
    }
}

#[test]
fn computed_guard_completion_executes_only_the_selected_target_call() {
    for form in 0..3 {
        let dispatch = dispatch(
            "identity(identity(flag)) && true",
            "(abort())",
            "(trap())",
            form,
        );
        let source = format!(
            r#"
            machine identity(input: bool) -> bool
            requires true == true
            ensures true == true
            {{ input }}
            machine abort() -> bool crashes Abort {{ crash Abort; }}
            machine trap() -> bool crashes Trap {{ crash Trap; }}
            machine value(flag: bool) -> bool
            requires true == true
            ensures true == true
            crashes Abort
            crashes Trap
            {{ {dispatch} }}
        "#
        );
        let artifact = encoded_guards(&source, &[], 1);
        for (flag, expected) in [
            (true, terminal_psi::CrashCause::Abort),
            (false, terminal_psi::CrashCause::Trap),
        ] {
            assert!(
                matches!(execute(&artifact, &[TerminalScalarValue::Boolean(flag)]),
                Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == expected)
            );
        }
    }
}

#[test]
fn computed_guards_distinguish_saved_values_and_updated_storage() {
    for form in 0..3 {
        let dispatch = dispatch(
            "identity(current) && !identity(saved)",
            "true",
            "false",
            form,
        );
        let source = format!(
            r#"
            machine identity(input: bool) -> bool
            requires true == true
            ensures true == true
            {{ input }}
            machine value(flag: bool) -> bool
            requires true == true
            ensures true == true
            {{
                let mut current: bool = identity(flag) || false;
                let saved: bool = current;
                current = identity(!current) && true;
                {dispatch}
            }}
        "#
        );
        let artifact = encoded_guards(&source, &["current", "saved"], 1);
        for flag in [false, true] {
            assert_eq!(
                execute(&artifact, &[TerminalScalarValue::Boolean(flag)]).unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(!flag))
            );
        }
    }
}

#[test]
fn computed_guards_transfer_completed_values_to_only_the_selected_named_state() {
    for form in 0..3 {
        let dispatch = dispatch(
            "identity(identity(flag)) || false",
            "finish(flag)",
            "finish(!flag)",
            form,
        );
        let source = format!(
            r#"
            machine identity(input: bool) -> bool
            requires true == true
            ensures true == true
            {{ input }}
            machine value(flag: bool) -> bool
            requires true == true
            ensures true == true
            {{
                {dispatch}
                state finish(forwarded: bool) -> bool {{ forwarded }}
            }}
        "#
        );
        let artifact = encoded_guards(&source, &[], 2);
        for flag in [false, true] {
            assert_eq!(
                execute(&artifact, &[TerminalScalarValue::Boolean(flag)]).unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
            );
        }
    }
}

#[test]
fn computed_guards_select_value_returns_in_combined_continuation_form() {
    for form in 0..3 {
        let dispatch = dispatch("identity(flag) && identity(flag)", "7u8", "9u8", form);
        let source = format!(
            r#"
            machine identity(input: bool) -> bool
            requires true == true
            ensures true == true
            {{ input }}
            machine value(flag: bool) -> u8
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ {dispatch} }}
        "#
        );
        let artifact = encoded_guard_arms(&source, &[], 1, true);
        for flag in [false, true] {
            assert_eq!(
                execute(&artifact, &[TerminalScalarValue::Boolean(flag)]).unwrap(),
                TerminalExecutionResult::Scalar(unsigned(8, if flag { 7 } else { 9 })),
                "form={form}, combined=true, flag={flag}"
            );
        }
    }
}

#[test]
fn computed_guard_fallback_state_arguments_keep_later_cast_calls_in_operand_order() {
    for (first, second, expected) in [
        ("Abort", "Trap", terminal_psi::CrashCause::Abort),
        ("Trap", "Abort", terminal_psi::CrashCause::Trap),
    ] {
        for form in 0..3 {
            let dispatch = dispatch(
                "identity(flag) && true",
                "finish(0u16)",
                "finish(first() + (second() as u16 in Wrapping))",
                form,
            );
            let source = format!(
                r#"
                machine identity(input: bool) -> bool
                requires true == true
                ensures true == true
                {{ input }}
                machine first() -> u16 in Wrapping crashes {first} {{ crash {first}; }}
                machine second() -> u8 crashes {second} {{ crash {second}; }}
                machine value(flag: bool) -> u16 in Wrapping
                requires 0u16 == 0u16
                ensures 0u16 == 0u16
                crashes Abort
                crashes Trap
                {{
                    {dispatch}
                    state finish(result_value: u16 in Wrapping) -> u16 in Wrapping {{ result_value }}
                }}
            "#
            );
            let artifact = encoded_guards(&source, &[], 2);
            assert_eq!(
                execute(&artifact, &[TerminalScalarValue::Boolean(true)]).unwrap(),
                TerminalExecutionResult::Scalar(unsigned(16, 0)),
                "form={form}, unselected fallback"
            );
            assert!(
                matches!(execute(&artifact, &[TerminalScalarValue::Boolean(false)]),
                Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == expected),
                "form={form}, first={first}"
            );
        }
    }
}

#[test]
fn computed_guard_custody_mutations_reject_before_publication() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            transition {
                identity(flag) && identity(!flag) -> true
                _ -> false
            }
        }
    "#;
    let checked = checked_arms(source, false);
    assert_guard_roots(&checked, &[], 1);
    checked_trees_to_lowered_psi::lower_machine(&checked, "value").unwrap();
    let (handle, root) = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .find(|(_, root)| root.role == CheckedScalarExpressionRole::Guard)
        .unwrap();
    let root = root.clone();
    let call_handle = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(node.kind, CheckedScalarComputationKind::Call { .. }).then_some(handle)
        })
        .unwrap();
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == root.machine)
        .unwrap();
    let state = &checked.typed.machine_states(machine)[0];
    for mutation in 0..15 {
        let mut changed = checked.clone();
        let plans = &mut changed.facts.values.scalar_computations;
        match mutation {
            0 => {
                plans.roots.append(root.clone());
            }
            1 => plans.roots.get_mut(handle).root = arena::Handle::invalid(),
            2 => plans.roots.get_mut(handle).machine = symbols::SymbolHandle::invalid(),
            3 => plans.roots.get_mut(handle).state = symbols::SymbolHandle::invalid(),
            4 => plans.roots.get_mut(handle).statement_ordinal += 100,
            5 => plans.roots.get_mut(handle).role = CheckedScalarExpressionRole::Return,
            6 => plans.nodes.get_mut(root.root).authored_root = arena::Handle::invalid(),
            7 => {
                plans.nodes.get_mut(root.root).primitive_type =
                    typed_trees::types::PrimitiveType::U8
            }
            8 | 9 => {
                let StatementNode::Transition(transition) = &mut changed
                    .typed
                    .statement_table
                    .statements_mut(state.statement_nodes)[root.statement_ordinal as usize]
                else {
                    panic!("authored transition");
                };
                transition.guard = if mutation == 8 {
                    TransitionGuardNode::Always
                } else {
                    TransitionGuardNode::When(arena::Handle::invalid())
                };
            }
            10 | 11 => {
                let CheckedScalarComputationKind::Call {
                    source_call,
                    call_ordinal,
                    ..
                } = &mut plans.nodes.get_mut(call_handle).kind
                else {
                    unreachable!();
                };
                if mutation == 10 {
                    *source_call = arena::Handle::invalid();
                } else {
                    *call_ordinal += 99;
                }
            }
            12 => {
                // A valid nested operand is not the authored outer guard.
                let nested_expression = plans.nodes.get(call_handle).authored_root;
                assert!(
                    !nested_expression.is_valid(),
                    "only destination roots own authored identity"
                );
                plans.roots.get_mut(handle).root = call_handle;
            }
            13 => {
                let StatementNode::Transition(transition) = &checked
                    .typed
                    .statement_table
                    .statements(state.statement_nodes)[root.statement_ordinal as usize]
                else {
                    panic!("authored transition");
                };
                let typed_trees::statement::TransitionTargetNode::Value(expression) = checked
                    .typed
                    .statement_table
                    .transition_target(transition.target)
                else {
                    panic!("returned Boolean literal");
                };
                let StatementNode::Transition(transition) = &mut changed
                    .typed
                    .statement_table
                    .statements_mut(state.statement_nodes)[root.statement_ordinal as usize]
                else {
                    unreachable!();
                };
                transition.guard = TransitionGuardNode::When(*expression);
            }
            14 => {
                let StatementNode::Transition(first) = &checked
                    .typed
                    .statement_table
                    .statements(state.statement_nodes)[root.statement_ordinal as usize]
                else {
                    unreachable!()
                };
                let StatementNode::Transition(fallback) = &mut changed
                    .typed
                    .statement_table
                    .statements_mut(state.statement_nodes)[root.statement_ordinal as usize + 1]
                else {
                    unreachable!()
                };
                // An independent guard cannot inherit the old unconditional edge.
                fallback.guard = first.guard;
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
            "guard mutation={mutation}"
        );
    }
}
