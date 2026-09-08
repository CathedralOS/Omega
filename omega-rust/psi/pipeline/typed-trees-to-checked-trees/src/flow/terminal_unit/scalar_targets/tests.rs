use super::*;

const SOURCE: &str = r#"
machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
machine first(left: u64, right: u64) -> u64 { left }
machine enter(before: u64, slot: &mut u64, after: u64) -> u64 {
    let answer: u64 = first(stamp(&mut slot, before), stamp(&mut slot, after));
    answer
}
machine observe(output: &mut u64) {
    let answer: u64 = enter(7, &mut output, 11);
    output = answer;
}
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn machine_symbol(checked: &checked_trees::CheckedTrees, name: &str) -> SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap()
        .symbol
}

#[test]
fn mixed_scalar_graph_retains_authored_positions_and_unit_call_custody() {
    let checked = checked(SOURCE);
    let enter = machine_symbol(&checked, "enter");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(enter)
        .unwrap();
    let [state] = graph.states.as_slice() else {
        panic!("one authored scalar state");
    };
    assert_eq!(
        state.parameter_types,
        [PrimitiveType::U64, PrimitiveType::U64]
    );
    assert_eq!(
        state
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.source_position)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(state.structural_parameters.len(), 1);
    assert_eq!(state.structural_parameters[0].position, 1);
    assert_eq!(
        state.structural_parameters[0].access,
        CheckedStructuralAccess::MutableBorrow
    );
    assert!(state.parameter_storage.is_empty());
    assert_eq!(state.bindings.len(), 1);
    assert_eq!(
        state.bindings[0].value,
        CheckedScalarBindingValue::Computation
    );
    let root = checked
        .facts
        .values
        .scalar_computations
        .root_at(
            state.state,
            0,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        )
        .expect("whole initializer computation");
    assert_eq!(root.machine, enter);
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(enter)
            .is_none()
    );
    let observer = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_symbol(&checked, "observe"))
        .expect("ordinary Unit caller retains the graph");
    let operation = observer.operations.iter().find(|operation| matches!(operation,
        CheckedUnitEffectOperationPlan::ScalarCall { target_machine, .. } if *target_machine == enter
    )).unwrap();
    assert!(is_available(
        &checked.typed,
        &checked.facts,
        observer,
        operation
    ));
}

#[test]
fn mixed_graph_target_rejects_stale_parameter_and_shape_namespaces() {
    let checked = checked(SOURCE);
    let enter = machine_symbol(&checked, "enter");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(enter)
        .unwrap();
    let state = graph.states[0].state;
    assert!(
        registered_primitive_graph_target(
            &checked.typed,
            &checked.facts,
            enter,
            state,
            PrimitiveType::U64
        )
        .is_some()
    );
    for mutation in 0..6 {
        let mut facts = checked.facts.clone();
        let graph = facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == enter)
            .unwrap();
        match mutation {
            0 => graph.states[0].scalar_parameters[1].source_position = 1,
            1 => graph.states[0].structural_parameters[0].position = 2,
            2 => graph.states[0].parameter_types[0] = PrimitiveType::Bool,
            3 => {
                graph.states[0].structural_parameters[0].access =
                    CheckedStructuralAccess::SharedBorrow
            }
            4 => {
                let _ = graph.states[0].scalar_parameters.pop();
            }
            5 => graph.states[0].result_type = PrimitiveType::Bool,
            _ => unreachable!(),
        }
        assert!(
            registered_primitive_graph_target(
                &checked.typed,
                &facts,
                enter,
                state,
                PrimitiveType::U64
            )
            .is_none(),
            "mutation {mutation}"
        );
    }
    let mut facts = checked.facts.clone();
    facts.flow.terminal_scalar_graphs.structural_types.clear();
    assert!(
        registered_primitive_graph_target(&checked.typed, &facts, enter, state, PrimitiveType::U64)
            .is_none()
    );
    let mut facts = checked.facts.clone();
    facts
        .flow
        .terminal_scalar_graphs
        .machines
        .push(graph.clone());
    assert!(
        registered_primitive_graph_target(&checked.typed, &facts, enter, state, PrimitiveType::U64)
            .is_none()
    );
}

#[test]
fn primitive_reference_leaves_keep_their_existing_body_owner() {
    let checked = checked(
        r#"
        machine hold(first: &u64, value: u64, second: &u64) -> u64 { value }
        machine reset(value: &mut u64) -> u64 { value = 0; 0 }
    "#,
    );
    for name in ["hold", "reset"] {
        let symbol = machine_symbol(&checked, name);
        assert!(
            checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(symbol)
                .is_none()
        );
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .for_machine(symbol)
                .is_some()
        );
    }
}

#[test]
fn scalar_only_graph_keeps_dense_rows_and_mutable_parameter_storage() {
    let checked =
        checked("machine enter(mut value: u64, other: u64) -> u64 { value = other; value }");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine_symbol(&checked, "enter"))
        .unwrap();
    let state = &graph.states[0];
    assert!(state.structural_parameters.is_empty());
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .is_empty()
    );
    assert_eq!(
        state
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.source_position)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        state.parameter_types,
        [PrimitiveType::U64, PrimitiveType::U64]
    );
    let storage = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .parameter_storage
        .span(state.parameter_storage)
        .unwrap();
    assert_eq!(storage.len(), 1);
    assert_eq!(storage[0].parameter_ordinal, 0);
}
