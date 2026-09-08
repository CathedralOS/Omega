use super::super::*;
use checked_trees::{
    CheckedStructuralControlTransferSourcePlan, CheckedStructuralScalarArgumentSourcePlan,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
};
use typed_trees::types::PrimitiveType;

const SOURCE: &str = r#"
machine reset(value: &mut u64) -> u64 { value = 0; 0 }
data Limits { limit: u64; divisor: u64 [3..=5]; }
machine walk(remaining: u64 [0..=5], limits: Limits, marker: u64)
terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);
-> u64 {
    let mut scratch: u64 = 0;
    transition remaining > 0 {
        true -> walk(remaining - 1, limits, reset(&mut scratch))
        false -> remaining
    }
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
    crate::lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
}

fn machine(checked: &checked_trees::CheckedTrees) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .unwrap()
        .symbol
}

fn successor(graph: &CheckedScalarMachineGraph) -> &CheckedScalarSuccessor {
    let CheckedScalarStateTerminator::Conditional {
        when_true: CheckedScalarBranchDestination::Jump(successor),
        when_false: CheckedScalarBranchDestination::Return { .. },
        ..
    } = &graph.states[0].terminator
    else {
        panic!("guarded self-loop and return")
    };
    successor
}

#[test]
fn owned_countdown_retains_mixed_positions_and_exact_nat_judgment() {
    for ranked in [true, false] {
        let source = if ranked {
            SOURCE.to_owned()
        } else {
            SOURCE.replace("terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);", "")
        };
        let checked = checked(&source);
        let machine = machine(&checked);
        let plans = &checked.facts.flow.terminal_scalar_graphs;
        let discovered = build_checked_scalar_graph_plans(
            &checked,
            &checked.facts.values.scalar_expressions,
            &checked.facts.values.scalar_computations,
        );
        assert!(
            discovered.for_machine(machine).is_some(),
            "source-shape discovery must normalize a machine-name backedge"
        );
        let graph = plans
            .for_machine(machine)
            .expect("one authored cyclic scalar graph");
        assert_eq!(graph.states.len(), 1);
        let state = &graph.states[0];
        assert_eq!(state.primitive_locals.len(), 1);
        assert_eq!(
            state.structural_parameters[0].multiplicity,
            Multiplicity::Affine
        );
        assert_eq!(state.structural_parameters[0].position, 1);
        assert_eq!(
            state
                .scalar_parameters
                .iter()
                .map(|parameter| parameter.source_position)
                .collect::<Vec<_>>(),
            [0, 2]
        );
        let successor = successor(graph);
        assert_eq!(successor.target, state.state);
        assert_ne!(
            successor.target, machine,
            "the graph names the entry child, not the authored machine target"
        );
        assert_eq!(successor.statement_ordinal, 1);
        assert_eq!(successor.argument_count, 3);
        let transfers = plans
            .structural_transfers
            .span(successor.structural_transfers)
            .unwrap();
        assert_eq!(transfers.len(), 1);
        assert_eq!(
            transfers[0].source,
            CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
        );
        assert_eq!(transfers[0].target_parameter_index, 0);
        let scalar = plans
            .scalar_arguments
            .span(successor.scalar_arguments)
            .unwrap();
        assert_eq!(
            scalar
                .iter()
                .map(|row| (row.argument_ordinal, row.target_scalar_parameter_index))
                .collect::<Vec<_>>(),
            [(0, 0), (2, 1)]
        );
        assert!(
            scalar
                .iter()
                .all(|row| row.source == CheckedStructuralScalarArgumentSourcePlan::Expression)
        );
        assert!(
            checked
                .facts
                .values
                .scalar_computations
                .root_at(
                    state.state,
                    1,
                    checked_trees::CheckedScalarExpressionRole::TransitionArgument {
                        argument_ordinal: 2
                    }
                )
                .is_some()
        );
        assert_eq!(graph.ranked_scc.is_some(), ranked);
        if let Some(rank) = &graph.ranked_scc {
            assert_eq!(rank.header_state, state.state);
            assert_eq!(rank.rank_scalar_parameter_index, 0);
            assert_eq!(rank.rank_primitive_type, PrimitiveType::U64);
            let [edge] = rank.covered_cyclic_edges.as_slice() else {
                panic!("one authored ranked edge")
            };
            assert_eq!(edge.statement_ordinal, successor.statement_ordinal);
            assert_eq!(edge.source_state, state.state);
            assert_eq!(edge.target_state, state.state);
        }
        let permissions = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == machine && event.access == PermissionAccess::Owned
            })
            .map(|(_, event)| (event.kind, event.source))
            .collect::<Vec<_>>();
        assert_eq!(
            permissions,
            [
                (
                    PermissionEventKind::Transfer,
                    PermissionEventSource::Call {
                        statement_index: 1,
                        call_ordinal: 0,
                        target_symbol: machine
                    }
                ),
                (
                    PermissionEventKind::AffineDrop,
                    PermissionEventSource::StateExit
                ),
            ]
        );
        let mut retained = plans.clone();
        finalize_checked_scalar_graph_plans(
            &checked,
            &checked.facts.flow.ownership,
            &checked.facts.values.scalar_computations,
            &mut retained,
        );
        assert!(retained.for_machine(machine).is_some());
    }
}

#[test]
fn cyclic_successor_rejects_substituted_argument_rows_and_rank_coordinates() {
    let checked = checked(SOURCE);
    let machine = machine(&checked);
    let plans = &checked.facts.flow.terminal_scalar_graphs;
    let original = successor(plans.for_machine(machine).unwrap());
    for mutation in 0..8 {
        let mut plans = plans.clone();
        match mutation {
            0 => {
                plans
                    .structural_transfers
                    .span_mut(original.structural_transfers)
                    .unwrap()[0]
                    .source = CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 }
            }
            1 => {
                plans
                    .structural_transfers
                    .span_mut(original.structural_transfers)
                    .unwrap()[0]
                    .target_parameter_index = 1
            }
            2 => {
                plans
                    .scalar_arguments
                    .span_mut(original.scalar_arguments)
                    .unwrap()[1]
                    .argument_ordinal = 1
            }
            3 => {
                plans
                    .scalar_arguments
                    .span_mut(original.scalar_arguments)
                    .unwrap()[1]
                    .target_scalar_parameter_index = 0
            }
            4 => {
                plans
                    .scalar_arguments
                    .span_mut(original.scalar_arguments)
                    .unwrap()[0]
                    .primitive_type = PrimitiveType::Bool
            }
            5 => {
                plans
                    .machines
                    .iter_mut()
                    .find(|graph| graph.machine == machine)
                    .unwrap()
                    .ranked_scc = None
            }
            6 => {
                plans
                    .machines
                    .iter_mut()
                    .find(|graph| graph.machine == machine)
                    .unwrap()
                    .ranked_scc
                    .as_mut()
                    .unwrap()
                    .rank_scalar_parameter_index = 1
            }
            _ => {
                plans
                    .machines
                    .iter_mut()
                    .find(|graph| graph.machine == machine)
                    .unwrap()
                    .ranked_scc
                    .as_mut()
                    .unwrap()
                    .covered_cyclic_edges[0]
                    .statement_ordinal = 2
            }
        }
        finalize_checked_scalar_graph_plans(
            &checked,
            &checked.facts.flow.ownership,
            &checked.facts.values.scalar_computations,
            &mut plans,
        );
        assert!(plans.for_machine(machine).is_none(), "mutation {mutation}");
    }
}

#[test]
fn cyclic_owned_permissions_reject_missing_duplicate_or_substituted_transfer_and_discard() {
    let checked = checked(SOURCE);
    let machine = machine(&checked);
    for kind in [
        PermissionEventKind::Transfer,
        PermissionEventKind::AffineDrop,
    ] {
        let handle = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .find(|(_, event)| {
                event.machine_symbol == machine
                    && event.access == PermissionAccess::Owned
                    && event.kind == kind
            })
            .unwrap()
            .0;
        for mutation in 0..10 {
            let mut ownership = checked.facts.flow.ownership.clone();
            match mutation {
                0 => {
                    ownership.permissions.get_mut(handle).machine_symbol =
                        symbols::SymbolHandle::invalid()
                }
                1 => ownership.permissions.get_mut(handle).root = facts::PlaceRoot::Unknown,
                2 => {
                    ownership.permissions.get_mut(handle).source =
                        PermissionEventSource::Statement { statement_index: 2 }
                }
                3 => ownership.permissions.get_mut(handle).multiplicity = Multiplicity::Linear,
                4 => ownership.permissions.get_mut(handle).obligation_live = true,
                5 => ownership.permissions.get_mut(handle).access = PermissionAccess::Shared,
                6 => {
                    let duplicate = ownership.permissions.get(handle).clone();
                    ownership.permissions.append(duplicate);
                }
                7 => {
                    ownership.permissions.get_mut(handle).source = PermissionEventSource::Call {
                        statement_index: 1,
                        call_ordinal: 1,
                        target_symbol: machine,
                    }
                }
                8 => {
                    ownership.permissions.get_mut(handle).source = PermissionEventSource::Call {
                        statement_index: 1,
                        call_ordinal: 0,
                        target_symbol: checked
                            .facts
                            .flow
                            .terminal_scalar_graphs
                            .for_machine(machine)
                            .unwrap()
                            .states[0]
                            .state,
                    }
                }
                _ => ownership.permissions.get_mut(handle).kind = PermissionEventKind::Consume,
            }
            let mut plans = checked.facts.flow.terminal_scalar_graphs.clone();
            finalize_checked_scalar_graph_plans(
                &checked,
                &ownership,
                &checked.facts.values.scalar_computations,
                &mut plans,
            );
            assert!(
                plans.for_machine(machine).is_none(),
                "{kind:?} mutation {mutation}"
            );
        }
    }
}

#[test]
fn pure_scalar_successors_also_retain_the_complete_scalar_partition() {
    let checked = checked(
        "machine walk(value: u64, marker: u64) -> u64 {
        transition value > 0 { true -> walk(value - 1, marker) false -> value }
    }",
    );
    let plans = &checked.facts.flow.terminal_scalar_graphs;
    let edge = successor(plans.for_machine(machine(&checked)).unwrap());
    assert_eq!(edge.argument_count, 2);
    assert!(
        plans
            .structural_transfers
            .span(edge.structural_transfers)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        plans
            .scalar_arguments
            .span(edge.scalar_arguments)
            .unwrap()
            .iter()
            .map(|row| (row.argument_ordinal, row.target_scalar_parameter_index))
            .collect::<Vec<_>>(),
        [(0, 0), (1, 1)]
    );
}

#[test]
fn cyclic_owned_signature_does_not_erase_mutability_linearity_or_qualifications() {
    let checked = checked(SOURCE);
    let machine = machine(&checked);
    let source_machine = checked
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .unwrap();
    let state = &checked.machine_states(source_machine)[0];
    let owned_symbol = checked.state_parameters(state)[1].symbol;
    let parameter_handle = checked
        .state_parameters
        .iter()
        .find(|(_, parameter)| parameter.symbol == owned_symbol)
        .unwrap()
        .0;
    let definition_handle = checked
        .data_definitions
        .iter()
        .find(|(_, definition)| definition.name.as_str() == "Limits")
        .unwrap()
        .0;
    for mutation in 0..3 {
        let mut program = checked.typed.clone();
        match mutation {
            0 => {
                program
                    .state_parameters
                    .get_mut(parameter_handle)
                    .is_mutable = true
            }
            1 => {
                program
                    .data_definitions
                    .get_mut(definition_handle)
                    .properties
                    .multiplicity = Multiplicity::Linear
            }
            _ => {
                let original = program
                    .state_parameters
                    .get(parameter_handle)
                    .type_reference;
                let qualified = program.type_reference_table.insert(
                    typed_trees::types::TypeReferenceNode::Constrained {
                        base_type: original,
                        constraints: arena::HandleSpan::empty(),
                    },
                );
                program
                    .state_parameters
                    .get_mut(parameter_handle)
                    .type_reference = qualified;
            }
        }
        let plans = build_checked_scalar_graph_plans(
            &program,
            &checked.facts.values.scalar_expressions,
            &checked.facts.values.scalar_computations,
        );
        assert!(plans.for_machine(machine).is_none(), "mutation {mutation}");
    }
}

#[test]
fn cyclic_owned_signature_rejects_direct_and_nested_nominal_cleanup() {
    for source in [
        format!("{SOURCE}\nmachine Limits::drop(&mut self) {{}}"),
        format!(
            "{}\nmachine Payload::drop(&mut self) {{}}",
            SOURCE.replace(
                "data Limits {",
                "data Payload { value: u64; } data Limits { payload: Payload;"
            )
        ),
    ] {
        let checked = checked(&source);
        assert!(
            checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine(&checked))
                .is_none()
        );
    }
}
