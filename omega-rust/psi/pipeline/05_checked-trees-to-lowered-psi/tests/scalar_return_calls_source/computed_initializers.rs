use super::checked_arms;
use checked_trees::CheckedScalarExpressionRole;
use checked_trees_to_lowered_psi::TerminalMachineSelection;

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
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("value"))
        .unwrap();
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
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("value")
                )
                .is_err(),
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
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("value")
            )
            .is_err(),
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
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("value")
            )
            .is_err(),
            "coordinated graph and root-role change cannot alter authored mutability {:?}",
            root.role
        );
    }
}
