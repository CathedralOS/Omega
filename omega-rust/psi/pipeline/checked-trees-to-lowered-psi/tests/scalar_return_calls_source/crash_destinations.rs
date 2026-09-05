use super::*;

#[test]
fn scalar_dispatch_executes_only_its_selected_crash_fallback() {
    for (cause, expected) in [
        ("Trap", terminal_psi::CrashCause::Trap),
        ("Abort", terminal_psi::CrashCause::Abort),
    ] {
        let source = format!(
            "machine value(flag: bool) -> u8
             requires 0u8 == 0u8
             ensures 0u8 == 0u8
             crashes {cause}
             {{ transition {{ flag -> 7u8 }} crash {cause}; }}"
        );
        let artifact = encoded(&source);
        assert_eq!(
            execute(&artifact, &[TerminalScalarValue::Boolean(true)]).unwrap(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: IntegerValue::Unsigned(7),
            })
        );
        let Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) =
            execute(&artifact, &[TerminalScalarValue::Boolean(false)])
        else {
            panic!("selected crash fallback");
        };
        assert_eq!(crash.cause, expected);
        assert!(crash.frontier_lower_bound.is_empty());
    }
}

fn assert_crash(
    result: Result<TerminalExecutionResult, TerminalArtifactInterpretError>,
    expected: terminal_psi::CrashCause,
) {
    let Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) =
        result
    else {
        panic!("expected selected {expected:?}");
    };
    assert_eq!(crash.cause, expected);
    assert!(crash.frontier_lower_bound.is_empty());
}

#[test]
fn computed_guard_crash_fallback_preserves_current_and_saved_values() {
    for (cause, expected) in [
        ("Trap", terminal_psi::CrashCause::Trap),
        ("Abort", terminal_psi::CrashCause::Abort),
    ] {
        let source = format!(
            r#"
            machine identity(input: bool) -> bool
            requires true == true
            ensures true == true
            {{ input }}
            machine value(flag: bool) -> bool
            requires true == true
            ensures true == true
            crashes {cause}
            {{
                let mut current: bool = identity(flag) || false;
                let saved: bool = current;
                current = identity(!current) && true;
                transition {{ identity(current) && !identity(saved) -> (identity(current)) }}
                crash {cause};
            }}
        "#
        );
        let artifact = encoded(&source);
        assert_eq!(
            execute(&artifact, &[TerminalScalarValue::Boolean(false)]).unwrap(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        );
        assert_crash(
            execute(&artifact, &[TerminalScalarValue::Boolean(true)]),
            expected,
        );
    }
}

#[test]
fn crash_fallback_is_skipped_after_pure_or_computed_named_state_selection() {
    for predicate in ["flag", "identity(identity(flag)) && true"] {
        for (cause, expected) in [
            ("Trap", terminal_psi::CrashCause::Trap),
            ("Abort", terminal_psi::CrashCause::Abort),
        ] {
            let source = format!(
                r#"
                machine identity(input: bool) -> bool
                requires true == true
                ensures true == true
                {{ input }}
                machine value(flag: bool) -> bool
                requires true == true
                ensures true == true
                crashes {cause}
                {{
                    transition {{ {predicate} -> finish(!flag) }}
                    crash {cause};
                    state finish(input: bool) -> bool {{ input }}
                }}
            "#
            );
            let artifact = encoded(&source);
            assert_eq!(
                execute(&artifact, &[TerminalScalarValue::Boolean(true)]).unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false))
            );
            assert_crash(
                execute(&artifact, &[TerminalScalarValue::Boolean(false)]),
                expected,
            );
        }
    }
}

#[test]
fn selected_guard_crash_precedes_the_direct_crash_fallback() {
    for (guard_cause, fallback_cause, expected_guard, expected_fallback) in [
        (
            "Abort",
            "Trap",
            terminal_psi::CrashCause::Abort,
            terminal_psi::CrashCause::Trap,
        ),
        (
            "Trap",
            "Abort",
            terminal_psi::CrashCause::Trap,
            terminal_psi::CrashCause::Abort,
        ),
    ] {
        for operator in ["&&", "||"] {
            let source = format!(
                r#"
                machine effect() -> bool crashes {guard_cause} {{ crash {guard_cause}; }}
                machine value(flag: bool) -> bool
                requires true == true
                ensures true == true
                crashes Abort
                crashes Trap
                {{
                    transition {{ flag {operator} effect() -> true }}
                    crash {fallback_cause};
                }}
            "#
            );
            let artifact = encoded(&source);
            let evaluates_effect = operator == "&&";
            assert_crash(
                execute(&artifact, &[TerminalScalarValue::Boolean(evaluates_effect)]),
                expected_guard,
            );
            let skipped = execute(
                &artifact,
                &[TerminalScalarValue::Boolean(!evaluates_effect)],
            );
            if operator == "&&" {
                assert_crash(skipped, expected_fallback);
            } else {
                assert_eq!(
                    skipped.unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
                );
            }
        }
    }
}

#[test]
fn parameter_relative_guard_routes_remain_distinct_from_direct_fallback_crashes() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine maybe_crash(input: bool) -> bool
        requires true == true
        ensures true == true
        crashes Abort
            input
        {
            transition input { true -> fail() false -> false }
            state fail() -> bool { crash Abort; }
        }
        machine value(selected: bool, flag: bool) -> bool
        requires true == true
        ensures true == true
        crashes Abort
        crashes Trap
        {
            transition { selected || maybe_crash(identity(flag)) -> true }
            crash Trap;
        }
    "#;
    let artifact = encoded(source);
    for selected in [false, true] {
        for flag in [false, true] {
            let result = execute(
                &artifact,
                &[
                    TerminalScalarValue::Boolean(selected),
                    TerminalScalarValue::Boolean(flag),
                ],
            );
            if selected {
                assert_eq!(
                    result.unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
                );
            } else {
                assert_crash(
                    result,
                    if flag {
                        terminal_psi::CrashCause::Abort
                    } else {
                        terminal_psi::CrashCause::Trap
                    },
                );
            }
        }
    }
}

#[test]
fn direct_fallback_routes_keep_entry_parameters_ahead_of_prior_local_values() {
    for predicate in ["flag", "flag || (identity(current) && false)"] {
        let source = format!(
            r#"
            machine identity(input: bool) -> bool
            requires true == true
            ensures true == true
            {{ input }}
            machine value(flag: bool, input: bool) -> bool
            requires true == true
            ensures true == true
            crashes Trap
                !flag
            {{
                let mut current: bool = input;
                let saved: bool = current;
                current = identity(!current);
                transition {{ {predicate} -> saved }}
                crash Trap;
            }}
        "#
        );
        let artifact = encoded(&source);
        for flag in [false, true] {
            for input in [false, true] {
                let result = execute(
                    &artifact,
                    &[
                        TerminalScalarValue::Boolean(flag),
                        TerminalScalarValue::Boolean(input),
                    ],
                );
                if flag {
                    assert_eq!(
                        result.unwrap(),
                        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(input))
                    );
                } else {
                    assert_crash(result, terminal_psi::CrashCause::Trap);
                }
            }
        }
    }
}

fn custody_source(computed: bool) -> String {
    let predicate = if computed {
        "identity(flag) && true"
    } else {
        "flag"
    };
    format!(
        r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        {{ input }}
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        crashes Trap
        {{ transition {{ {predicate} -> true }} crash Trap; }}
    "#
    )
}

#[test]
fn direct_crash_causes_require_matching_routes_during_independent_verification() {
    for computed in [false, true] {
        let artifact = encoded(&custody_source(computed));
        let mut module = terminal_codec::decode_module(&artifact.0).unwrap();
        let machine = module
            .machines
            .iter_mut()
            .find(|machine| {
                machine
                    .blocks
                    .iter()
                    .any(|block| matches!(block.terminator, terminal_psi::Terminator::Crash { .. }))
            })
            .expect("machine owning the fallback");
        let cause = machine
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator {
                terminal_psi::Terminator::Crash { cause, .. } => Some(cause),
                _ => None,
            })
            .expect("direct fallback crash");
        *cause = terminal_psi::CrashCause::Abort;
        assert!(
            matches!(
                encode_module(&module),
                Err(terminal_codec::CodecError::InvalidModule(
                    terminal_verifier::ModuleError::CrashRouteUncovered {
                        cause: terminal_psi::CrashCause::Abort,
                        ..
                    }
                ))
            ),
            "uncovered changed cause rejects before encoding"
        );
        let machine = module
            .machines
            .iter_mut()
            .find(|machine| {
                machine
                    .blocks
                    .iter()
                    .any(|block| matches!(block.terminator, terminal_psi::Terminator::Crash { .. }))
            })
            .unwrap();
        assert_eq!(machine.contract.crash_routes.len(), 1);
        machine.contract.crash_routes[0].cause = terminal_psi::CrashCause::Abort;
        // These proof rows establish unchanged Boolean tautologies, not the
        // authenticity of an entire semantic module. Reuse is valid when the
        // verifier reconstructs a consistent body and published crash route.
        let changed = (encode_module(&module).unwrap(), artifact.1);
        assert_eq!(
            execute(&changed, &[TerminalScalarValue::Boolean(true)]).unwrap(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
            "unchanged successful branch, computed={computed}"
        );
        assert_crash(
            execute(&changed, &[TerminalScalarValue::Boolean(false)]),
            terminal_psi::CrashCause::Abort,
        );
    }
}

#[test]
fn direct_crash_fallback_custody_mutations_reject_before_publication() {
    use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};

    for computed in [false, true] {
        let checked = checked_arms(&custody_source(computed), false);
        checked_trees_to_lowered_psi::lower_machine(&checked, "value").unwrap();
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "value")
            .unwrap();
        let [state] = checked.typed.machine_states(machine) else {
            panic!("no synthetic source states");
        };
        let statements = checked
            .typed
            .statement_table
            .statements(state.statement_nodes);
        let [
            StatementNode::Transition(selected),
            StatementNode::Transition(fallback),
        ] = statements
        else {
            panic!("exactly the authored transition and crash, no hoisted locals");
        };
        assert_eq!(
            fallback.exit,
            TransitionExit::Crash(typed_trees::signature::CrashCause::Trap)
        );
        for mutation in 0..10 {
            let mut changed = checked.clone();
            if mutation == 9 {
                let StatementNode::Transition(selected) = &mut changed
                    .typed
                    .statement_table
                    .statements_mut(state.statement_nodes)[0]
                else {
                    unreachable!();
                };
                selected.guard = TransitionGuardNode::Always;
            } else if mutation < 6 {
                let StatementNode::Transition(fallback) = &mut changed
                    .typed
                    .statement_table
                    .statements_mut(state.statement_nodes)[1]
                else {
                    unreachable!();
                };
                match mutation {
                    0 => {
                        fallback.exit =
                            TransitionExit::Crash(typed_trees::signature::CrashCause::Abort)
                    }
                    1 => fallback.exit = TransitionExit::Ordinary,
                    2 => fallback.target = selected.target,
                    3 => fallback.target = arena::Handle::invalid(),
                    4 => fallback.continuation = selected.target,
                    5 => fallback.guard = selected.guard,
                    _ => unreachable!(),
                }
            } else if mutation == 6 {
                let contract = changed
                    .facts
                    .contract_plans
                    .machines
                    .iter_mut()
                    .find(|contract| contract.machine == machine.symbol)
                    .unwrap();
                contract.crash = contract
                    .crash
                    .clone()
                    .with_checked_sites(Vec::new())
                    .unwrap();
            } else {
                let graph = changed
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .machines
                    .iter_mut()
                    .find(|graph| graph.machine == machine.symbol)
                    .unwrap();
                let graph_state = graph
                    .states
                    .iter_mut()
                    .find(|candidate| candidate.state == state.symbol)
                    .unwrap();
                let checked_trees::CheckedScalarStateTerminator::Conditional { when_false, .. } =
                    &mut graph_state.terminator
                else {
                    panic!("conditional crash fallback");
                };
                let checked_trees::CheckedScalarBranchDestination::Crash { statement_ordinal } =
                    when_false
                else {
                    panic!("exact crash destination");
                };
                *statement_ordinal = if mutation == 7 { 100 } else { 0 };
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "fallback mutation={mutation}, computed={computed}"
            );
        }
        assert_eq!(fallback.guard, TransitionGuardNode::Always);
    }
}
