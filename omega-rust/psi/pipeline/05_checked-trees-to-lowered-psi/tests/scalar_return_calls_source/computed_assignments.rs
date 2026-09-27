use super::{
    IntegerSign, IntegerType, IntegerValue, TerminalExecutionResult, TerminalScalarValue,
    checked_arms, encode_module, encode_proof_section, execute,
};
use checked_trees::{CheckedScalarBindingDestination, CheckedScalarExpressionRole};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
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
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("value"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
}

fn unsigned(bits: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
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
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("value"))
        .unwrap();
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
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("value")
                )
                .is_err(),
                "mutation={mutation}, statement={}",
                root.statement_ordinal
            );
        }
    }
}
