use super::*;
use checked_trees::CheckedScalarExpressionRole;

fn assert_initializer_roots(
    checked: &checked_trees::CheckedTrees,
    names: &[&str],
    state_count: usize,
) {
    assert_selected_initializer_roots(checked, names, names, state_count);
}

fn assert_selected_initializer_roots(
    checked: &checked_trees::CheckedTrees,
    names: &[&str],
    computed_names: &[&str],
    state_count: usize,
) {
    use typed_trees::statement::StatementNode;

    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let states = checked.typed.machine_states(machine);
    assert_eq!(states.len(), state_count, "no manufactured source state");
    let mut locals = Vec::new();
    for state in states {
        for (statement_ordinal, statement) in checked
            .typed
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            if let StatementNode::LocalData(local) = statement {
                locals.push(local.name.as_str());
                assert!(
                    local.initial_value.is_valid(),
                    "authored initializer retained"
                );
                if !computed_names.contains(&local.name.as_str()) {
                    continue;
                }
                let roots: Vec<_> = checked
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .filter(|(_, root)| {
                        root.machine == machine.symbol
                            && root.state == state.symbol
                            && root.statement_ordinal == statement_ordinal as u32
                            && matches!(
                                root.role,
                                CheckedScalarExpressionRole::LocalInitializer { .. }
                                    | CheckedScalarExpressionRole::StorageInitializer
                            )
                    })
                    .collect();
                assert_eq!(roots.len(), 1, "one root for {}", local.name.as_str());
                let root = roots[0].1;
                assert_eq!(
                    root.role == CheckedScalarExpressionRole::StorageInitializer,
                    local.is_mutable,
                    "initializer destination agrees with source storage"
                );
                assert_eq!(
                    checked
                        .facts
                        .values
                        .scalar_computations
                        .nodes
                        .get(root.root)
                        .authored_root,
                    local.initial_value,
                    "initializer root belongs to the authored LocalData"
                );
            }
        }
    }
    assert_eq!(locals, names, "no hoisted source temporaries");
}

fn encoded_initializers(source: &str, names: &[&str], state_count: usize) -> (Vec<u8>, Vec<u8>) {
    let checked = checked_arms(source, false);
    assert_initializer_roots(&checked, names, state_count);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).expect("encode semantics"),
        encode_proof_bundle(&lowered.proof_bundle).expect("encode proof"),
    )
}

fn unsigned(bits: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn computed_initializers_feed_later_initializers_and_returns() {
    let source = r#"
        machine identity(input: u8 in Wrapping) -> u8 in Wrapping
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        { input }
        machine value(input: u8 in Wrapping) -> u8 in Wrapping
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        {
            let first: u8 in Wrapping = identity(input) + 1u8;
            let second: u8 in Wrapping = identity(first) * 2u8;
            second - first
        }
    "#;
    let artifact = encoded_initializers(source, &["first", "second"], 1);
    for input in [0, 7, 127, 254, 255] {
        assert_eq!(
            execute(&artifact, &[unsigned(8, input)]).unwrap(),
            TerminalExecutionResult::Scalar(unsigned(8, (input + 1) % 256))
        );
    }
}

#[test]
fn computed_storage_initializers_do_not_rebind_saved_values_after_overwrite() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            let mut current: bool = identity(flag) && true;
            let saved: bool = identity(current) || false;
            current = !flag;
            saved && !current
        }
    "#;
    let artifact = encoded_initializers(source, &["current", "saved"], 1);
    for flag in [false, true] {
        let input = TerminalScalarValue::Boolean(flag);
        assert_eq!(
            execute(&artifact, &[input]).unwrap(),
            TerminalExecutionResult::Scalar(input)
        );
    }
}

#[test]
fn bare_mutable_call_initializers_establish_current_storage() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            let mut current: bool = identity(flag);
            current
        }
    "#;
    let artifact = encoded_initializers(source, &["current"], 1);
    for flag in [false, true] {
        let input = TerminalScalarValue::Boolean(flag);
        assert_eq!(
            execute(&artifact, &[input]).unwrap(),
            TerminalExecutionResult::Scalar(input)
        );
    }
}

#[test]
fn nested_call_initializers_preserve_each_completed_argument() {
    for qualifier in ["", "mut "] {
        let source = format!(
            r#"
            machine identity(input: u8 in Wrapping) -> u8 in Wrapping
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ input }}
            machine value(input: u8 in Wrapping) -> u8 in Wrapping
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{
                let {qualifier}saved: u8 in Wrapping = identity(identity(input) + 1u8);
                saved
            }}
        "#
        );
        let artifact = encoded_initializers(&source, &["saved"], 1);
        for input in [0, 7, 254, 255] {
            assert_eq!(
                execute(&artifact, &[unsigned(8, input)]).unwrap(),
                TerminalExecutionResult::Scalar(unsigned(8, (input + 1) % 256))
            );
        }
    }
}

#[test]
fn mixed_initializer_namespaces_keep_pure_call_and_storage_bindings_distinct() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            let pure: bool = !flag;
            let direct: bool = identity(flag);
            let mut current: bool = identity(pure) && true;
            let saved: bool = current;
            current = direct;
            let result_value: bool = identity(saved) && !identity(current);
            result_value
        }
    "#;
    let checked = checked_arms(source, false);
    assert_selected_initializer_roots(
        &checked,
        &["pure", "direct", "current", "saved", "result_value"],
        &["current", "result_value"],
        1,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter()
            .flat_map(|machine| &machine.states)
            .flat_map(|state| &state.bindings)
            .any(|binding| matches!(
                binding.value,
                checked_trees::CheckedScalarBindingValue::DirectCall { .. }
            )),
        "legacy direct call shares the rebased local namespace"
    );
    let artifact = encoded(source);
    for flag in [false, true] {
        assert_eq!(
            execute(&artifact, &[TerminalScalarValue::Boolean(flag)]).unwrap(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(!flag))
        );
    }
}

#[test]
fn computed_initializers_skip_only_unselected_boolean_operands() {
    for (operator, skipped) in [("&&", false), ("||", true)] {
        for mutable in [false, true] {
            let qualifier = if mutable { "mut " } else { "" };
            let source = format!(
                r#"
                machine effect() -> bool crashes Abort {{ crash Abort; }}
                machine value(flag: bool) -> bool
                requires true == true
                ensures true == true
                crashes Abort
                {{
                    let {qualifier}saved: bool = flag {operator} effect();
                    saved
                }}
            "#
            );
            let artifact = encoded_initializers(&source, &["saved"], 1);
            assert_eq!(
                execute(&artifact, &[TerminalScalarValue::Boolean(skipped)]).unwrap(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(skipped))
            );
            assert!(matches!(
                execute(&artifact, &[TerminalScalarValue::Boolean(!skipped)]),
                Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)))
                    if crash.cause == terminal_psi::CrashCause::Abort
            ));
        }
    }
}

#[test]
fn computed_initializer_operands_crash_in_authored_order_despite_later_cast() {
    for (first, second, expected) in [
        ("Abort", "Trap", terminal_psi::CrashCause::Abort),
        ("Trap", "Abort", terminal_psi::CrashCause::Trap),
    ] {
        for mutable in [false, true] {
            let qualifier = if mutable { "mut " } else { "" };
            let source = format!(
                r#"
                machine first() -> u16 in Wrapping crashes {first} {{ crash {first}; }}
                machine second() -> u8 crashes {second} {{ crash {second}; }}
                machine value() -> u16 in Wrapping
                requires 0u16 == 0u16
                ensures 0u16 == 0u16
                crashes Abort
                crashes Trap
                {{
                    let {qualifier}saved: u16 in Wrapping = first() + (second() as u16 in Wrapping);
                    saved
                }}
            "#
            );
            let artifact = encoded_initializers(&source, &["saved"], 1);
            assert!(matches!(execute(&artifact, &[]),
                Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)))
                    if crash.cause == expected
            ));
        }
    }
}

#[test]
fn computed_initializers_preserve_integer_policies_and_cast_carriers() {
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
            {{
                let saved: {result_type} = {expression};
                saved
            }}
        "#
        );
        let artifact = encoded_initializers(&source, &["saved"], 1);
        assert_eq!(
            execute(&artifact, &[unsigned(8, 255)]).unwrap(),
            TerminalExecutionResult::Scalar(unsigned(bits, expected)),
            "{policy}: {expression}"
        );
    }
}

#[test]
fn computed_initializers_transfer_completed_values_to_named_states() {
    let source = r#"
        machine identity(input: u8 in Wrapping) -> u8 in Wrapping
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        { input }
        machine value(input: u8 in Wrapping) -> u8 in Wrapping
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        {
            let saved: u8 in Wrapping = identity(input) + 1u8;
            transition { _ -> finish(saved) }
            state finish(forwarded: u8 in Wrapping) -> u8 in Wrapping {
                let next: u8 in Wrapping = identity(forwarded) * 2u8;
                next
            }
        }
    "#;
    let artifact = encoded_initializers(source, &["saved", "next"], 2);
    for input in [0, 7, 127, 254, 255] {
        assert_eq!(
            execute(&artifact, &[unsigned(8, input)]).unwrap(),
            TerminalExecutionResult::Scalar(unsigned(8, ((input + 1) * 2) % 256))
        );
    }
}

fn narrowing_source() -> &'static str {
    r#"
        machine bounded(input: u16) -> u16
        requires input < 256u16
        ensures result == input
        { input }
        machine value(input: u16) -> u8
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        {
            let saved: u8 = bounded(input % 256u16) as u8;
            saved
        }
    "#
}

#[test]
fn computed_initializer_narrowing_uses_actual_contracted_arguments() {
    let artifact = encoded_initializers(narrowing_source(), &["saved"], 1);
    for input in [0, 7, 255, 256, 263, 65535] {
        assert_eq!(
            execute(&artifact, &[unsigned(16, input)]).unwrap(),
            TerminalExecutionResult::Scalar(unsigned(8, input % 256))
        );
    }
}

#[test]
fn computed_initializer_narrowing_rejects_changed_guarantees_with_stale_proof() {
    use semantic_vocabulary::{Proposition, ScalarTerm};

    let artifact = encoded_initializers(narrowing_source(), &["saved"], 1);
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
        .expect("result is tied to the evaluated argument");
    guarantee.proposition = Proposition::Truth;
    let changed = (encode_module(&module).unwrap(), artifact.1);
    assert!(execute(&changed, &[unsigned(16, 263)]).is_err());
}

#[test]
fn computed_initializer_custody_mutations_reject_before_publication() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            let saved: bool = identity(flag) && true;
            let mut current: bool = identity(!flag) || false;
            saved && !current
        }
    "#;
    let checked = checked_arms(source, false);
    assert_initializer_roots(&checked, &["saved", "current"], 1);
    checked_trees_to_lowered_psi::lower_machine(&checked, "value").unwrap();
    let roots: Vec<_> = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| {
            matches!(
                root.role,
                CheckedScalarExpressionRole::LocalInitializer { .. }
                    | CheckedScalarExpressionRole::StorageInitializer
            )
        })
        .map(|(handle, root)| (handle, root.clone()))
        .collect();
    assert_eq!(roots.len(), 2);
    for (handle, root) in &roots {
        let opposite = &roots.iter().find(|(other, _)| other != handle).unwrap().1;
        for mutation in 0..9 {
            let mut changed = checked.clone();
            let plans = &mut changed.facts.values.scalar_computations;
            match mutation {
                0 => {
                    plans.roots.append(root.clone());
                }
                1 => plans.roots.get_mut(*handle).root = arena::Handle::invalid(),
                2 => plans.roots.get_mut(*handle).role = CheckedScalarExpressionRole::Return,
                3 => {
                    plans.roots.get_mut(*handle).role =
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: 99,
                        }
                }
                4 => plans.roots.get_mut(*handle).statement_ordinal += 100,
                5 => plans.nodes.get_mut(root.root).authored_root = arena::Handle::invalid(),
                6 => plans.roots.get_mut(*handle).root = opposite.root,
                7 => plans.roots.get_mut(*handle).role = opposite.role,
                8 => {
                    plans.nodes.get_mut(root.root).primitive_type =
                        typed_trees::types::PrimitiveType::U8;
                    let graph = changed
                        .facts
                        .flow
                        .terminal_scalar_graphs
                        .machines
                        .iter_mut()
                        .find(|graph| graph.machine == root.machine)
                        .unwrap();
                    let state = graph
                        .states
                        .iter_mut()
                        .find(|state| state.state == root.state)
                        .unwrap();
                    let binding = state
                        .bindings
                        .iter_mut()
                        .find(|binding| binding.statement_ordinal == root.statement_ordinal)
                        .unwrap();
                    binding.primitive_type = typed_trees::types::PrimitiveType::U8;
                }
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "mutation={mutation}, role={:?}",
                root.role
            );
        }
    }

    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.typed.machine_states(machine)[0];
    let parameter = checked.typed.state_parameters(state)[0].symbol;
    let (storage_handle, storage_root) = roots
        .iter()
        .find(|(_, root)| root.role == CheckedScalarExpressionRole::StorageInitializer)
        .unwrap();
    for symbol in [symbols::SymbolHandle::invalid(), parameter] {
        let mut changed = checked.clone();
        let graph = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == storage_root.machine)
            .unwrap();
        let state = graph
            .states
            .iter_mut()
            .find(|state| state.state == storage_root.state)
            .unwrap();
        let binding = state
            .bindings
            .iter_mut()
            .find(|binding| binding.statement_ordinal == storage_root.statement_ordinal)
            .unwrap();
        binding.destination =
            checked_trees::CheckedScalarBindingDestination::StorageInitialize { symbol };
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
            "computed storage initializer rejects nonlocal destination {symbol:?}"
        );
    }

    for (handle, root) in &roots {
        let typed_trees::statement::StatementNode::LocalData(local) = &checked
            .typed
            .statement_table
            .statements(state.statement_nodes)[root.statement_ordinal as usize]
        else {
            panic!("authored local");
        };
        let (role, destination) = if handle == storage_handle {
            (
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
                checked_trees::CheckedScalarBindingDestination::Immutable,
            )
        } else {
            (
                CheckedScalarExpressionRole::StorageInitializer,
                checked_trees::CheckedScalarBindingDestination::StorageInitialize {
                    symbol: local.symbol,
                },
            )
        };
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .roots
            .get_mut(*handle)
            .role = role;
        let graph = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == root.machine)
            .unwrap();
        let state = graph
            .states
            .iter_mut()
            .find(|state| state.state == root.state)
            .unwrap();
        let binding = state
            .bindings
            .iter_mut()
            .find(|binding| binding.statement_ordinal == root.statement_ordinal)
            .unwrap();
        binding.destination = destination;
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
            "coordinated graph and root-role change cannot alter authored mutability {:?}",
            root.role
        );
    }
}
