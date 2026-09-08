use super::super::build_checked_scalar_graph_plans;
use checked_trees::{
    CheckedScalarBindingDestination, CheckedScalarBindingValue, CheckedScalarComputationKind,
    CheckedScalarExpressionRole, CheckedScalarPrimitiveLocalPlan, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape,
};
use typed_trees::{statement::StatementNode, types::PrimitiveType};

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

fn source(parameters: &str, initializer: &str) -> String {
    format!(
        r#"
machine stamp(value: &mut u64, number: u64) -> u64 {{ value = number; number }}
machine stamp_byte(value: &mut u8, number: u8) -> u8 {{ value = number; number }}
machine touch(value: &mut bool) -> bool {{ value = true; true }}
machine identity(value: u64) -> u64 {{ value }}
machine first(left: u64, right: u64) -> u64 {{ left }}
machine enter({parameters}) -> u64 {{
    let mut unused: u64 = 99;
    let mut slot: u64 = {initializer};
    let mut narrow: u8 = 0u8;
    let mut selected: bool = false;
    let answer: u64 = first(stamp(&mut slot, 7), stamp(&mut slot, 11));
    let byte_answer: u8 = stamp_byte(&mut narrow, 5u8);
    let selection: bool = flag && touch(&mut selected);
    let snapshot: u64 = slot;
    slot = answer;
    snapshot
}}
"#
    )
}

#[test]
fn borrowed_scalar_locals_retain_exact_roster_and_normalized_shapes() {
    for parameters in ["flag: bool", "external: &mut u64, flag: bool"] {
        for initializer in ["201", "identity(201)"] {
            let checked = checked(&source(parameters, initializer));
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "enter")
                .unwrap();
            let source_state = &checked.machine_states(machine)[0];
            let plans = &checked.facts.flow.terminal_scalar_graphs;
            let graph = plans
                .for_machine(machine.symbol)
                .expect("local borrow graph");
            let state = &graph.states[0];
            let statements = checked
                .statement_table
                .statements(source_state.statement_nodes);
            let expected = [
                (1, PrimitiveType::U64),
                (2, PrimitiveType::U8),
                (3, PrimitiveType::Bool),
            ]
            .into_iter()
            .map(|(ordinal, primitive_type)| {
                let StatementNode::LocalData(local) = &statements[ordinal] else {
                    panic!("local declaration")
                };
                CheckedScalarPrimitiveLocalPlan {
                    statement_ordinal: ordinal as u32,
                    symbol: local.symbol,
                    primitive_type,
                    type_identity: checked
                        .normalized_type_identity_with_binders_and_substitutions(
                            local.type_reference,
                            &[],
                            &[],
                        )
                        .into_string(),
                }
            })
            .collect::<Vec<_>>();
            assert_eq!(state.primitive_locals, expected);
            for local in &expected {
                assert_eq!(
                    plans
                        .structural_types
                        .iter()
                        .filter(|shape| shape.identity == local.type_identity)
                        .collect::<Vec<_>>(),
                    [&CheckedUnitStructuralTypePlan {
                        identity: local.type_identity.clone(),
                        shape: CheckedUnitStructuralTypeShape::PrimitiveScalar(
                            local.primitive_type
                        )
                    }]
                );
                assert_eq!(
                    state.bindings[local.statement_ordinal as usize].destination,
                    CheckedScalarBindingDestination::StorageInitialize {
                        symbol: local.symbol
                    }
                );
            }
            assert_eq!(
                state.bindings[1].value,
                if initializer == "201" {
                    CheckedScalarBindingValue::Expression
                } else {
                    CheckedScalarBindingValue::Computation
                }
            );
            assert!(
                matches!(state.bindings[8].destination, CheckedScalarBindingDestination::StorageAssign { symbol } if symbol == expected[0].symbol)
            );
            assert_eq!(
                state.structural_parameters.len(),
                usize::from(parameters.starts_with("external"))
            );
        }
    }
}

#[test]
fn scalar_locals_without_borrow_demand_keep_existing_scalar_bindings() {
    let checked =
        checked("machine enter(value: u64) -> u64 { let mut slot: u64 = value; slot = 7; slot }");
    let plans = &checked.facts.flow.terminal_scalar_graphs;
    let graph = plans.for_machine(checked.machines()[0].symbol).unwrap();
    assert!(graph.states[0].primitive_locals.is_empty());
    assert!(plans.structural_types.is_empty());
    assert_eq!(graph.states[0].bindings.len(), 2);
}

#[test]
fn orphan_computations_do_not_create_local_places() {
    let checked = checked(&source("flag: bool", "201"));
    let mut computations = checked.facts.values.scalar_computations.clone();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let StatementNode::LocalData(unused) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("unused local")
    };
    let mut orphan = computations.nodes.iter().find_map(|(_, node)| matches!(&node.kind, CheckedScalarComputationKind::Call { structural_arguments, .. } if !structural_arguments.is_empty()).then_some(node.clone())).unwrap();
    let CheckedScalarComputationKind::Call {
        structural_arguments,
        ..
    } = &mut orphan.kind
    else {
        panic!("borrowed call")
    };
    let mut argument = computations
        .structural_arguments
        .span(*structural_arguments)
        .unwrap()[0]
        .clone();
    argument.source = checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
        symbol: unused.symbol,
    };
    *structural_arguments = computations.structural_arguments.insert_many([argument]);
    computations.nodes.append(orphan);
    assert_eq!(
        build_checked_scalar_graph_plans(
            &checked,
            &checked.facts.values.scalar_expressions,
            &computations
        ),
        checked.facts.flow.terminal_scalar_graphs
    );
}

#[test]
fn borrowed_local_rejects_missing_or_mismatched_initializer_facts() {
    let checked = checked(&source("flag: bool", "201"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let state = checked.machine_states(machine)[0].symbol;
    for mutation in 0..5 {
        let mut expressions = checked.facts.values.scalar_expressions.clone();
        let role = CheckedScalarExpressionRole::StorageInitializer;
        let source_handle = expressions
            .source_bindings
            .iter()
            .find(|(_, binding)| {
                binding.state == state && binding.statement_ordinal == 1 && binding.role == role
            })
            .unwrap()
            .0;
        match mutation {
            0 => expressions.expressions.retain(|expression| {
                !(expression.state == state
                    && expression.statement_ordinal == 1
                    && expression.role == role)
            }),
            1 => {
                expressions
                    .source_bindings
                    .get_mut(source_handle)
                    .destination = symbols::SymbolHandle::invalid()
            }
            2 => {
                expressions
                    .source_bindings
                    .get_mut(source_handle)
                    .expression = arena::Handle::invalid()
            }
            3 => {
                let expression = expressions
                    .expressions
                    .iter_mut()
                    .find(|expression| {
                        expression.state == state
                            && expression.statement_ordinal == 1
                            && expression.role == role
                    })
                    .unwrap();
                expression.expression = checked_trees::CheckedScalarExpression::Boolean(Box::new(
                    checked_trees::CheckedBooleanExpression::Constant(false),
                ));
            }
            _ => {
                expressions
                    .source_bindings
                    .append(expressions.source_bindings.get(source_handle).clone());
            }
        }
        let plans = build_checked_scalar_graph_plans(
            &checked,
            &expressions,
            &checked.facts.values.scalar_computations,
        );
        assert!(
            plans.for_machine(machine.symbol).is_none(),
            "initializer mutation {mutation}"
        );
    }
}

#[test]
fn borrowed_local_rejects_mismatched_computed_initializer() {
    let checked = checked(&source("flag: bool", "identity(201)"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let state = checked.machine_states(machine)[0].symbol;
    for mutation in 0..3 {
        let mut computations = checked.facts.values.scalar_computations.clone();
        let root = computations
            .root_at(state, 1, CheckedScalarExpressionRole::StorageInitializer)
            .unwrap()
            .root;
        match mutation {
            0 => computations.nodes.get_mut(root).primitive_type = PrimitiveType::U8,
            1 => computations.nodes.get_mut(root).authored_root = arena::Handle::invalid(),
            _ => {
                let root_handle = computations
                    .roots
                    .iter()
                    .find(|(_, row)| row.root == root)
                    .unwrap()
                    .0;
                computations.roots.get_mut(root_handle).root = arena::Handle::invalid();
            }
        }
        let plans = build_checked_scalar_graph_plans(
            &checked,
            &checked.facts.values.scalar_expressions,
            &computations,
        );
        assert!(
            plans.for_machine(machine.symbol).is_none(),
            "computed initializer mutation {mutation}"
        );
    }
}

#[test]
fn return_and_guard_computations_retain_borrowed_local_demand() {
    for ending in [
        "stamp(&mut slot, 7) == 7",
        "transition flag && (stamp(&mut slot, 7) == 7) { true -> true false -> false }",
    ] {
        let checked = checked(&format!(
            "machine stamp(value: &mut u64, number: u64) -> u64 {{ value = number; number }}
             machine enter(flag: bool) -> bool {{ let mut slot: u64 = 0; {ending} }}"
        ));
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "enter")
            .unwrap();
        let state = &checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine.symbol)
            .unwrap()
            .states[0];
        assert_eq!(state.primitive_locals.len(), 1);
        assert_eq!(state.primitive_locals[0].statement_ordinal, 0);
        assert_eq!(state.primitive_locals[0].primitive_type, PrimitiveType::U64);
        assert!(
            checked
                .facts
                .values
                .scalar_computations
                .nodes
                .iter()
                .any(|(_, node)| matches!(node.kind, CheckedScalarComputationKind::Apply { .. }))
        );
    }
}

#[test]
fn borrowed_local_demand_rejects_wrong_referent_or_type() {
    let checked = checked(&source("flag: bool", "201"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let statements = checked.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(slot) = &statements[1] else {
        panic!("slot")
    };
    let StatementNode::LocalData(snapshot) = &statements[7] else {
        panic!("snapshot")
    };
    for mutation in 0..4 {
        let mut computations = checked.facts.values.scalar_computations.clone();
        let handles = computations.structural_arguments.iter().filter_map(|(handle, argument)| matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } if symbol == slot.symbol).then_some(handle)).collect::<Vec<_>>();
        assert!(!handles.is_empty());
        for handle in handles {
            let argument = computations.structural_arguments.get_mut(handle);
            match mutation {
                0 => {
                    argument.source =
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                            symbol: symbols::SymbolHandle::invalid(),
                        }
                }
                1 => {
                    argument.source =
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                            symbol: snapshot.symbol,
                        }
                }
                2 => argument.type_identity = "wrong referent".to_owned(),
                _ => argument.access = checked_trees::CheckedStructuralAccess::Owned,
            }
        }
        let plans = build_checked_scalar_graph_plans(
            &checked,
            &checked.facts.values.scalar_expressions,
            &computations,
        );
        assert!(
            plans.for_machine(machine.symbol).is_none(),
            "borrow demand mutation {mutation}"
        );
    }
}

#[test]
fn borrowed_local_graphs_do_not_admit_multiple_source_states() {
    let checked = checked(
        "machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
         machine enter() -> u64 {
             let mut slot: u64 = 0;
             let answer: u64 = stamp(&mut slot, 7);
             transition { _ -> finish(answer) }
             state finish(value: u64) -> u64 { value }
         }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    assert!(
        checked
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .iter()
            .any(|(_, argument)| matches!(
                argument.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
            ))
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine.symbol)
            .is_none()
    );
}
