use super::*;
use checked_trees::{CheckedScalarBindingDestination, CheckedScalarExpressionRole};
use typed_trees::statement::StatementNode;

fn assert_assignment_roots(
    checked: &checked_trees::CheckedTrees,
    local_names: &[&str],
    state_count: usize,
    assignment_count: usize,
) {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let states = checked.typed.machine_states(machine);
    assert_eq!(states.len(), state_count, "no manufactured source states");
    let mut names = Vec::new();
    let mut assignments = 0;
    for state in states {
        for (ordinal, statement) in checked
            .typed
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            match statement {
                StatementNode::LocalData(local) => names.push(local.name.as_str()),
                StatementNode::Assignment(assignment) => {
                    assignments += 1;
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
                                && root.role == CheckedScalarExpressionRole::AssignmentValue
                        })
                        .collect();
                    assert_eq!(roots.len(), 1, "one authored assignment root at {ordinal}");
                    assert_eq!(
                        checked
                            .facts
                            .values
                            .scalar_computations
                            .nodes
                            .get(roots[0].1.root)
                            .authored_root,
                        assignment.value,
                        "the root is the original assignment RHS"
                    );
                }
                _ => {}
            }
        }
    }
    assert_eq!(names, local_names, "no hoisted source temporaries");
    assert_eq!(
        assignments, assignment_count,
        "authored assignments remain statements"
    );
}

fn encoded_assignments(
    source: &str,
    names: &[&str],
    states: usize,
    assignments: usize,
) -> (Vec<u8>, Vec<u8>) {
    let checked = checked_arms(source, false);
    assert_assignment_roots(&checked, names, states, assignments);
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
fn repeated_computed_assignments_use_current_storage_and_preserve_saved_values() {
    let source = r#"
        machine identity(input: u8 in Wrapping) -> u8 in Wrapping
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        { input }
        machine value(input: u8 in Wrapping) -> u8 in Wrapping
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        {
            let mut current: u8 in Wrapping = input;
            let saved: u8 in Wrapping = current;
            current = identity(identity(current) + 1u8);
            current = identity(current) * 2u8;
            let difference: u8 in Wrapping = identity(current) - identity(saved);
            difference
        }
    "#;
    let artifact = encoded_assignments(source, &["current", "saved", "difference"], 1, 2);
    for input in [0, 7, 127, 254, 255] {
        assert_eq!(
            execute(&artifact, &[unsigned(8, input)]).unwrap(),
            TerminalExecutionResult::Scalar(unsigned(8, (input + 2) % 256))
        );
    }
}

#[test]
fn bare_assignment_calls_replace_storage_before_named_state_transfer() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            let mut current: bool = flag;
            current = identity(!current);
            transition { _ -> finish(current) }
            state finish(forwarded: bool) -> bool {
                let mut next: bool = forwarded;
                next = identity(identity(!next));
                next
            }
        }
    "#;
    let artifact = encoded_assignments(source, &["current", "next"], 2, 2);
    for flag in [false, true] {
        let input = TerminalScalarValue::Boolean(flag);
        assert_eq!(
            execute(&artifact, &[input]).unwrap(),
            TerminalExecutionResult::Scalar(input)
        );
    }
}

#[test]
fn computed_assignments_skip_unselected_crashing_boolean_operands() {
    for (operator, skipped) in [("&&", false), ("||", true)] {
        for (cause, expected) in [
            ("Abort", terminal_psi::CrashCause::Abort),
            ("Trap", terminal_psi::CrashCause::Trap),
        ] {
            let source = format!(
                r#"
                machine effect() -> bool crashes {cause} {{ crash {cause}; }}
                machine value(flag: bool) -> bool
                requires true == true
                ensures true == true
                crashes {cause}
                {{
                    let mut current: bool = flag;
                    current = current {operator} effect();
                    current
                }}
            "#
            );
            let artifact = encoded_assignments(&source, &["current"], 1, 1);
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

#[test]
fn computed_assignment_operands_crash_left_to_right_before_later_casts() {
    for (first, second, expected) in [
        ("Abort", "Trap", terminal_psi::CrashCause::Abort),
        ("Trap", "Abort", terminal_psi::CrashCause::Trap),
    ] {
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
                let mut current: u16 in Wrapping = 0u16;
                current = first() + (second() as u16 in Wrapping);
                current
            }}
        "#
        );
        let artifact = encoded_assignments(&source, &["current"], 1, 1);
        assert!(matches!(execute(&artifact, &[]),
            Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == expected));
    }
}

#[test]
fn computed_assignments_preserve_operand_policies_and_widening() {
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
                let mut current: {result_type} = 0u{bits};
                current = {expression};
                current
            }}
        "#
        );
        let artifact = encoded_assignments(&source, &["current"], 1, 1);
        assert_eq!(
            execute(&artifact, &[unsigned(8, 255)]).unwrap(),
            TerminalExecutionResult::Scalar(unsigned(bits, expected)),
            "{expression}"
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
            let mut current: u8 = 0u8;
            current = bounded(input % 256u16) as u8;
            current
        }
    "#
}

#[test]
fn computed_assignment_narrowing_uses_the_evaluated_callee_argument() {
    let artifact = encoded_assignments(narrowing_source(), &["current"], 1, 1);
    for input in [0, 7, 255, 256, 263, 65535] {
        assert_eq!(
            execute(&artifact, &[unsigned(16, input)]).unwrap(),
            TerminalExecutionResult::Scalar(unsigned(8, input % 256))
        );
    }
}

#[test]
fn assignment_destination_carrier_does_not_prove_unbounded_call_result_narrowing() {
    let source = r#"
        machine identity(input: u16) -> u16
        requires 0u16 == 0u16
        ensures result == input
        { input }
        machine value(input: u16) -> u8
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        {
            let mut current: u8 = 0u8;
            current = identity(input) as u8;
            current
        }
    "#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    match lower_typed_trees(typed) {
        Err(diagnostics) => assert!(!diagnostics.is_empty()),
        Ok(checked) => assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "value").is_err(),
            "the destination's u8 carrier cannot justify narrowing an arbitrary u16 result"
        ),
    }
}

#[test]
fn self_reading_assignment_narrowing_uses_the_value_before_each_write() {
    let source = r#"
        machine identity(input: u8) -> u8
        requires 0u8 == 0u8
        ensures 0u8 == 0u8
        { input }
        machine value(input: u16) -> u16
        requires 0u16 == 0u16
        ensures 0u16 == 0u16
        {
            let mut current: u16 = input;
            current = identity((current / 256u16) as u8) as u16;
            current = identity((current % 256u16) as u8) as u16;
            current
        }
    "#;
    let artifact = encoded_assignments(source, &["current"], 1, 2);
    for input in [0, 7, 255, 256, 263, 65280, 65535] {
        assert_eq!(
            execute(&artifact, &[unsigned(16, input)]).unwrap(),
            TerminalExecutionResult::Scalar(unsigned(16, input / 256)),
            "input={input}"
        );
    }
}

#[test]
fn computed_assignment_narrowing_rejects_changed_guarantees_with_stale_proof() {
    use semantic_vocabulary::{Proposition, ScalarTerm};

    let artifact = encoded_assignments(narrowing_source(), &["current"], 1, 1);
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
        .unwrap();
    guarantee.proposition = Proposition::Truth;
    assert!(
        execute(
            &(encode_module(&module).unwrap(), artifact.1),
            &[unsigned(16, 263)]
        )
        .is_err()
    );
}

#[test]
fn computed_assignment_custody_mutations_reject_before_publication() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            let mut current: bool = flag;
            let mut other: bool = !flag;
            current = identity(current) && true;
            other = identity(!other) || false;
            current && other
        }
    "#;
    let checked = checked_arms(source, false);
    assert_assignment_roots(&checked, &["current", "other"], 1, 2);
    checked_trees_to_lowered_psi::lower_machine(&checked, "value").unwrap();
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.typed.machine_states(machine)[0];
    let parameter = checked.typed.state_parameters(state)[0].symbol;
    let roots: Vec<_> = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| {
            root.machine == machine.symbol
                && root.role == CheckedScalarExpressionRole::AssignmentValue
        })
        .map(|(handle, root)| (handle, root.clone()))
        .collect();
    assert_eq!(roots.len(), 2);
    for (handle, root) in &roots {
        let opposite = &roots.iter().find(|(other, _)| other != handle).unwrap().1;
        for mutation in 0..15 {
            let mut changed = checked.clone();
            let plans = &mut changed.facts.values.scalar_computations;
            match mutation {
                0 => {
                    plans.roots.append(root.clone());
                }
                1 => plans.roots.get_mut(*handle).root = arena::Handle::invalid(),
                2 => plans.roots.get_mut(*handle).machine = symbols::SymbolHandle::invalid(),
                3 => plans.roots.get_mut(*handle).state = symbols::SymbolHandle::invalid(),
                4 => plans.roots.get_mut(*handle).statement_ordinal += 100,
                5 => {
                    plans.roots.get_mut(*handle).role =
                        CheckedScalarExpressionRole::StorageInitializer
                }
                6 => plans.nodes.get_mut(root.root).authored_root = arena::Handle::invalid(),
                7 => plans.roots.get_mut(*handle).root = opposite.root,
                8 => {
                    let original = plans.nodes.get(opposite.root).authored_root;
                    let StatementNode::Assignment(assignment) = &mut changed
                        .typed
                        .statement_table
                        .statements_mut(state.statement_nodes)
                        [root.statement_ordinal as usize]
                    else {
                        panic!("assignment");
                    };
                    assignment.value = original;
                }
                9..=14 => {
                    if mutation == 13 {
                        plans.nodes.get_mut(root.root).primitive_type =
                            typed_trees::types::PrimitiveType::U8;
                    }
                    if mutation == 14 {
                        plans.roots.get_mut(*handle).role =
                            CheckedScalarExpressionRole::StorageInitializer;
                    }
                    let graph = changed
                        .facts
                        .flow
                        .terminal_scalar_graphs
                        .machines
                        .iter_mut()
                        .find(|graph| graph.machine == root.machine)
                        .unwrap();
                    let graph_state = graph
                        .states
                        .iter_mut()
                        .find(|state| state.state == root.state)
                        .unwrap();
                    let other_destination = graph_state
                        .bindings
                        .iter()
                        .find(|binding| binding.statement_ordinal == opposite.statement_ordinal)
                        .unwrap()
                        .destination;
                    let binding = graph_state
                        .bindings
                        .iter_mut()
                        .find(|binding| binding.statement_ordinal == root.statement_ordinal)
                        .unwrap();
                    match mutation {
                        9 => {
                            binding.destination = CheckedScalarBindingDestination::StorageAssign {
                                symbol: symbols::SymbolHandle::invalid(),
                            }
                        }
                        10 => {
                            binding.destination =
                                CheckedScalarBindingDestination::StorageAssign { symbol: parameter }
                        }
                        11 => binding.destination = other_destination,
                        12 | 13 => binding.primitive_type = typed_trees::types::PrimitiveType::U8,
                        14 => {
                            let CheckedScalarBindingDestination::StorageAssign { symbol } =
                                binding.destination
                            else {
                                panic!("storage assignment");
                            };
                            binding.destination =
                                CheckedScalarBindingDestination::StorageInitialize { symbol };
                        }
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "mutation={mutation}, statement={}",
                root.statement_ordinal
            );
        }
    }
}
