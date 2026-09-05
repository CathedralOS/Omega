use super::*;

fn encoded_computed_arms(source: &str, combined: bool) -> (Vec<u8>, Vec<u8>) {
    let checked = checked_arms(source, combined);
    assert_authored_return_roots(&checked, source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}, combined={combined}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).expect("encode semantics"),
        encode_proof_bundle(&lowered.proof_bundle).expect("encode proof"),
    )
}

fn assert_authored_return_roots(checked: &checked_trees::CheckedTrees, source: &str) {
    use checked_trees::CheckedScalarExpressionRole;

    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    assert_eq!(
        checked.typed.machine_states(machine).len(),
        1,
        "computed returns must not manufacture source continuation states"
    );
    assert!(
        checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, root)| root.machine == machine.symbol
                && matches!(
                    root.role,
                    CheckedScalarExpressionRole::Return
                        | CheckedScalarExpressionRole::ContinuationReturn
                )),
        "authored return must retain its checked computation root: {source}"
    );
}

fn unsigned(bits: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn guarded_computed_integer_returns_execute_the_selected_expression() {
    for (expression, expected) in [
        ("identity(input) + 1u8", 0),
        ("identity(input) - 1u8", 254),
        ("identity(input) * 2u8", 254),
        ("identity(input) / 3u8", 85),
        ("identity(input) % 3u8", 0),
        ("identity(input) & 15u8", 15),
        ("identity(input) | 0u8", 255),
        ("identity(input) ^ 15u8", 240),
        ("~identity(input)", 0),
        ("identity(input) << 1u16", 254),
        ("identity(input) >> 1u16", 127),
        ("identity(identity(input) - 1u8) - 2u8", 252),
    ] {
        let source = format!(
            r#"
            machine identity(input: u8 in Wrapping) -> u8 in Wrapping
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ input }}
            machine value(selected: bool, input: u8 in Wrapping) -> u8 in Wrapping
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{
                transition selected {{
                    true -> ({expression})
                    false -> 17u8
                }}
            }}
        "#
        );
        for combined in [false, true] {
            let artifact = encoded_computed_arms(&source, combined);
            for selected in [false, true] {
                assert_eq!(
                    execute(
                        &artifact,
                        &[TerminalScalarValue::Boolean(selected), unsigned(8, 255)],
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(unsigned(
                        8,
                        if selected { expected } else { 17 }
                    )),
                    "{expression}, selected={selected}, combined={combined}",
                );
            }
        }
    }
}

#[test]
fn direct_computed_returns_preserve_operand_policy_and_cast_carriers() {
    for (policy, result_type, bits, expression, expected) in [
        ("Wrapping", "u8 in Wrapping", 8, "identity(input) + 1", 0),
        (
            "Saturating",
            "u8 in Saturating",
            8,
            "identity(input) + 1",
            255,
        ),
        (
            "Wrapping",
            "u16",
            16,
            "(identity(input) as u16) + 1u16",
            256,
        ),
        (
            "Wrapping",
            "u8 in Saturating",
            8,
            "(identity(input) as u8 in Saturating) + 1u8",
            255,
        ),
        (
            "Saturating",
            "u8 in Wrapping",
            8,
            "(identity(input) as u8 in Wrapping) + 1u8",
            0,
        ),
    ] {
        let source = format!(
            r#"
            machine identity(input: u8 in {policy}) -> u8 in {policy}
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ input }}
            machine value(input: u8 in {policy}) -> {result_type}
            requires 0u{bits} == 0u{bits}
            ensures 0u{bits} == 0u{bits}
            {{ {expression} }}
        "#
        );
        let artifact = encoded_computed_arms(&source, false);
        assert_eq!(
            execute(&artifact, &[unsigned(8, 255)]).unwrap(),
            TerminalExecutionResult::Scalar(unsigned(bits, expected)),
            "{policy}: {expression}",
        );
    }
}

#[test]
fn direct_return_cast_does_not_hoist_a_later_crashing_operand() {
    for (first_cause, second_cause, expected) in [
        ("Abort", "Trap", terminal_psi::CrashCause::Abort),
        ("Trap", "Abort", terminal_psi::CrashCause::Trap),
    ] {
        let source = format!(
            r#"
            machine first() -> u16 in Wrapping
            crashes {first_cause}
            {{ crash {first_cause}; }}
            machine second() -> u8
            crashes {second_cause}
            {{ crash {second_cause}; }}
            machine value() -> u16 in Wrapping
            requires 0u16 == 0u16
            ensures 0u16 == 0u16
            crashes Abort
            crashes Trap
            {{ first() + (second() as u16 in Wrapping) }}
        "#
        );
        let artifact = encoded_computed_arms(&source, false);
        let result = execute(&artifact, &[]);
        assert!(
            matches!(result,
            Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)))
                if crash.cause == expected),
            "first={first_cause}, second={second_cause}"
        );
    }
}

#[test]
fn unconditional_transition_return_keeps_its_computed_cast_root() {
    let source = r#"
        machine identity(input: u8) -> u8
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        { input }
        machine value(input: u8) -> u16
        requires 0u16 == 0u16
        ensures 0u16 == 0u16
        {
            transition { _ -> ((identity(input) as u16) + 1u16) }
        }
    "#;
    let artifact = encoded_computed_arms(source, false);
    for input in [0, 7, 127, 255] {
        assert_eq!(
            execute(&artifact, &[unsigned(8, input)]).unwrap(),
            TerminalExecutionResult::Scalar(unsigned(16, input + 1)),
            "input={input}",
        );
    }
}

#[test]
fn guarded_computed_comparison_returns_keep_authored_operand_meaning() {
    for (operator, expected) in [
        ("==", false),
        ("!=", true),
        ("<", true),
        ("<=", true),
        (">", false),
        (">=", false),
    ] {
        let source = format!(
            r#"
            machine identity(input: u8) -> u8
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ input }}
            machine value(selected: bool, left: u8, right: u8) -> bool
            requires true == true
            ensures true == true
            {{
                transition selected {{
                    true -> (identity(left) {operator} identity(right))
                    false -> false
                }}
            }}
        "#
        );
        for combined in [false, true] {
            let artifact = encoded_computed_arms(&source, combined);
            for selected in [false, true] {
                assert_eq!(
                    execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            unsigned(8, 3),
                            unsigned(8, 9),
                        ]
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                        selected && expected
                    )),
                    "{operator}, selected={selected}, combined={combined}",
                );
            }
        }
    }
}

#[test]
fn computed_boolean_returns_skip_unselected_crashing_operands() {
    let source = r#"
        machine effect() -> bool
        crashes Abort
        { crash Abort; }
        machine value(selected: bool, flag: bool) -> bool
        requires true == true
        ensures true == true
        crashes Abort
        {
            transition selected {
                true -> (flag || effect())
                false -> (flag && effect())
            }
        }
    "#;
    for combined in [false, true] {
        let artifact = encoded_computed_arms(source, combined);
        for selected in [false, true] {
            for flag in [false, true] {
                let result = execute(
                    &artifact,
                    &[
                        TerminalScalarValue::Boolean(selected),
                        TerminalScalarValue::Boolean(flag),
                    ],
                );
                if selected == flag {
                    assert_eq!(
                        result.unwrap(),
                        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(flag))
                    );
                } else {
                    assert!(matches!(result,
                        Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)))
                            if crash.cause == terminal_psi::CrashCause::Abort));
                }
            }
        }
    }
}

#[test]
fn computed_return_operand_order_is_observable_at_the_first_crash() {
    for operator in [">", ">=", "==", "!="] {
        for (left, right, expected) in [
            ("abort", "trap", terminal_psi::CrashCause::Abort),
            ("trap", "abort", terminal_psi::CrashCause::Trap),
        ] {
            let source = format!(
                r#"
                machine abort() -> u8 in Wrapping crashes Abort {{ crash Abort; }}
                machine trap() -> u8 in Wrapping crashes Trap {{ crash Trap; }}
                machine value(selected: bool) -> bool
                requires true == true
                ensures true == true
                crashes Abort
                crashes Trap
                {{
                    transition selected {{
                        true -> (({left}() - 1u8) {operator} ({right}() + 1u8))
                        false -> false
                    }}
                }}
            "#
            );
            for combined in [false, true] {
                let artifact = encoded_computed_arms(&source, combined);
                assert_eq!(
                    execute(&artifact, &[TerminalScalarValue::Boolean(false)]).unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(false))
                );
                let result = execute(&artifact, &[TerminalScalarValue::Boolean(true)]);
                assert!(
                    matches!(result,
                    Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)))
                        if crash.cause == expected),
                    "{left} {operator} {right}, combined={combined}"
                );
            }
        }
    }
}

#[test]
fn computed_returns_keep_saved_values_distinct_from_current_storage() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine value(selected: bool, flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            let mut current: bool = flag;
            let saved: bool = current;
            current = !flag;
            transition selected {
                true -> (identity(current) && !identity(saved))
                false -> (identity(saved) || !identity(current))
            }
        }
    "#;
    for combined in [false, true] {
        let artifact = encoded_computed_arms(source, combined);
        for selected in [false, true] {
            for flag in [false, true] {
                assert_eq!(
                    execute(
                        &artifact,
                        &[
                            TerminalScalarValue::Boolean(selected),
                            TerminalScalarValue::Boolean(flag),
                        ]
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(if selected {
                        !flag
                    } else {
                        flag
                    }))
                );
            }
        }
    }
}

fn narrowing_source() -> &'static str {
    r#"
        machine bounded(input: u16) -> u16
        requires input < 256u16
        ensures result == input
        { input }
        machine value(selected: bool, input: u16) -> u8
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        {
            transition selected {
                true -> (bounded(input % 256u16) as u8)
                false -> 17u8
            }
        }
    "#
}

#[test]
fn computed_return_narrowing_uses_the_actual_contracted_argument() {
    for combined in [false, true] {
        let artifact = encoded_computed_arms(narrowing_source(), combined);
        for selected in [false, true] {
            for input in [0, 7, 255, 256, 263, 65535] {
                assert_eq!(
                    execute(
                        &artifact,
                        &[TerminalScalarValue::Boolean(selected), unsigned(16, input),]
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(unsigned(
                        8,
                        if selected { input % 256 } else { 17 }
                    ))
                );
            }
        }
    }
}

#[test]
fn computed_return_narrowing_rejects_a_weakened_guarantee_with_stale_proof() {
    use semantic_vocabulary::{Proposition, ScalarTerm};

    for combined in [false, true] {
        let artifact = encoded_computed_arms(narrowing_source(), combined);
        let mut module = terminal_codec::decode_module(&artifact.0).unwrap();
        let guarantee = module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.contract.ensures)
            .find(|clause| {
                matches!(
                    clause.proposition,
                    Proposition::Equal(ScalarTerm::Value { .. }, ScalarTerm::Value { .. })
                )
            })
            .expect("callee guarantee joins its result and argument");
        guarantee.proposition = Proposition::Truth;
        let changed = (encode_module(&module).unwrap(), artifact.1);
        for selected in [false, true] {
            assert!(
                execute(
                    &changed,
                    &[TerminalScalarValue::Boolean(selected), unsigned(16, 263),]
                )
                .is_err(),
                "selected={selected}, combined={combined}"
            );
        }
    }
}

#[test]
fn computed_return_custody_mutations_reject_before_publication() {
    use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpressionRole};

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
                true -> (flag && identity(flag))
                false -> (flag || identity(!flag))
            }
        }
    "#;
    for combined in [false, true] {
        let checked = checked_arms(source, combined);
        assert_authored_return_roots(&checked, source);
        checked_trees_to_lowered_psi::lower_machine(&checked, "value").unwrap();
        let plans = &checked.facts.values.scalar_computations;
        let roots: Vec<_> = plans
            .roots
            .iter()
            .map(|(handle, root)| (handle, root.clone()))
            .collect();
        assert_eq!(roots.len(), 2, "each authored arm owns its return root");
        if combined {
            assert!(
                roots
                    .iter()
                    .any(|(_, root)| root.role == CheckedScalarExpressionRole::ContinuationReturn)
            );
        } else {
            assert!(
                roots
                    .iter()
                    .all(|(_, root)| root.role == CheckedScalarExpressionRole::Return)
            );
        }
        let call_handle = plans
            .nodes
            .iter()
            .find_map(|(handle, node)| {
                matches!(node.kind, CheckedScalarComputationKind::Call { .. }).then_some(handle)
            })
            .expect("computed return call");
        assert!(
            roots
                .iter()
                .all(|(_, root)| plans.nodes.get(root.root).authored_root.is_valid())
        );
        assert_ne!(
            plans.nodes.get(roots[0].1.root).authored_root,
            plans.nodes.get(roots[1].1.root).authored_root,
            "each arm retains its own authored expression identity"
        );
        for (root_handle, root) in roots.iter().cloned() {
            let opposite_root = roots
                .iter()
                .find(|(handle, _)| *handle != root_handle)
                .unwrap()
                .1
                .root;
            for mutation in 0..10 {
                let mut changed = checked.clone();
                let plans = &mut changed.facts.values.scalar_computations;
                match mutation {
                    0 => {
                        plans.roots.append(root.clone());
                    }
                    1 => {
                        plans.roots.get_mut(root_handle).root = arena::Handle::invalid();
                    }
                    2 => {
                        plans.roots.get_mut(root_handle).role = match root.role {
                            CheckedScalarExpressionRole::Return => {
                                CheckedScalarExpressionRole::ContinuationReturn
                            }
                            CheckedScalarExpressionRole::ContinuationReturn => {
                                CheckedScalarExpressionRole::Return
                            }
                            _ => unreachable!(),
                        };
                    }
                    3 => {
                        plans.roots.get_mut(root_handle).machine = symbols::SymbolHandle::invalid();
                    }
                    4 => {
                        plans.roots.get_mut(root_handle).state = symbols::SymbolHandle::invalid();
                    }
                    5 => {
                        plans.roots.get_mut(root_handle).statement_ordinal += 100;
                    }
                    6 | 7 => {
                        let CheckedScalarComputationKind::Call {
                            source_call,
                            call_ordinal,
                            ..
                        } = &mut plans.nodes.get_mut(call_handle).kind
                        else {
                            unreachable!()
                        };
                        if mutation == 6 {
                            *source_call = arena::Handle::invalid();
                        } else {
                            *call_ordinal += 1;
                        }
                    }
                    8 => {
                        plans.nodes.get_mut(root.root).authored_root = arena::Handle::invalid();
                    }
                    9 => {
                        plans.nodes.get_mut(root.root).authored_root =
                            plans.nodes.get(opposite_root).authored_root;
                    }
                    _ => unreachable!(),
                }
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                    "mutation={mutation}, role={:?}, combined={combined}",
                    root.role
                );
            }
        }
        for swap_site in [false, true] {
            let mut changed = checked.clone();
            let plans = &mut changed.facts.values.scalar_computations;
            let [(first_handle, first), (second_handle, second)] = roots.as_slice() else {
                unreachable!()
            };
            if swap_site {
                // Combined arms share a statement and differ only by role.
                // Separate arms share Return and differ by statement ordinal.
                // Swapping both coordinates keeps both sites unique in either form.
                plans.roots.get_mut(*first_handle).role = second.role;
                plans.roots.get_mut(*first_handle).statement_ordinal = second.statement_ordinal;
                plans.roots.get_mut(*second_handle).role = first.role;
                plans.roots.get_mut(*second_handle).statement_ordinal = first.statement_ordinal;
            } else {
                plans.roots.get_mut(*first_handle).root = second.root;
                plans.roots.get_mut(*second_handle).root = first.root;
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "paired swap_site={swap_site}, combined={combined}"
            );
        }
    }
}
