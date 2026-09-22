//! Descriptor selections carried through checked dynamic parameter transfers.

use crate::execution::terminal_unit::{
    BTreeMap, CheckFacts, CheckedUnitCallCoordinate, ExpressionNode, SymbolHandle,
    TypeReferenceNode, TypedTrees, state_flow,
};

pub(super) fn build_checked_dynamic_descriptor_transfers(
    program: &TypedTrees,
    facts: &CheckFacts,
    binding_facts: &checked_trees::DynamicConformanceBindingFacts,
) -> Vec<checked_trees::CheckedDynamicDescriptorTransferPlan> {
    let mut transfers = Vec::new();
    let inbound_call_site_counts = inbound_call_site_counts(program, facts);
    loop {
        let mut changed = false;
        for caller in program.machines() {
            for caller_state in program.machine_states(caller) {
                let Some(flow) = state_flow(facts, caller.symbol, caller_state.symbol) else {
                    continue;
                };
                for call in facts.flow.control.calls.span_or_empty(flow.calls) {
                    let Some(call_site) = crate::semantic_calls::find_call_site(
                        program,
                        caller.symbol,
                        caller_state.symbol,
                        call.statement_index,
                        call.call_ordinal,
                    ) else {
                        continue;
                    };
                    let Some(target_state) =
                        crate::semantic_calls::find_state(program, call.target_symbol)
                    else {
                        continue;
                    };
                    let Some(target_machine) = program.machines().iter().find(|machine| {
                        program
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == target_state.symbol)
                    }) else {
                        continue;
                    };
                    let arguments =
                        crate::semantic_calls::call_site_argument_expressions(program, &call_site);
                    let parameters = program
                        .state_parameters(target_state)
                        .iter()
                        .filter(|parameter| !parameter.is_self)
                        .collect::<Vec<_>>();
                    if arguments.len() != parameters.len() {
                        continue;
                    }
                    for (parameter_position, (parameter, argument)) in
                        parameters.into_iter().zip(arguments).enumerate()
                    {
                        let coordinate = CheckedUnitCallCoordinate {
                            statement_index: match u32::try_from(call.statement_index) {
                                Ok(index) => index,
                                Err(_) => continue,
                            },
                            call_ordinal: match u32::try_from(call.call_ordinal) {
                                Ok(ordinal) => ordinal,
                                Err(_) => continue,
                            },
                        };
                        let Ok(parameter_position) = u32::try_from(parameter_position) else {
                            continue;
                        };
                        if transfers.iter().any(
                            |transfer: &checked_trees::CheckedDynamicDescriptorTransferPlan| {
                                transfer.caller_machine == caller.symbol
                                    && transfer.caller_state == caller_state.symbol
                                    && transfer.coordinate == coordinate
                                    && transfer.parameter_position == parameter_position
                            },
                        ) {
                            continue;
                        }
                        let Some(target_trait) =
                            bare_dynamic_parameter_trait(program, parameter.type_reference)
                        else {
                            continue;
                        };
                        let ExpressionNode::Name(source_path) =
                            program.expression_table.expression(*argument)
                        else {
                            continue;
                        };
                        let [source_name] = program
                            .expression_table
                            .name_path_members(source_path.members)
                        else {
                            continue;
                        };
                        let mut local_selections = binding_facts
                            .selections
                            .iter()
                            .filter(|selection| {
                                selection.machine == caller.symbol
                                    && selection.state == caller_state.symbol
                                    && selection.binding == source_path.symbol
                                    && selection.binding_name == *source_name
                                    && selection.statement_index < call.statement_index
                                    && selection.target_trait == target_trait
                            })
                            .collect::<Vec<_>>();
                        local_selections.sort_by_key(|selection| selection.statement_index);
                        let (source, source_predecessor_count, mut source_paths) =
                            if let Some(selection) = local_selections.last() {
                                (
                                checked_trees::CheckedDynamicDescriptorTransferSource::Selection,
                                0,
                                vec![checked_trees::CheckedDynamicDescriptorTransferPath {
                                    selection: (*selection).clone(),
                                    edges: Vec::new(),
                                }],
                            )
                            } else {
                                let source_parameters = program
                                    .state_parameters(caller_state)
                                    .iter()
                                    .filter(|parameter| !parameter.is_self)
                                    .collect::<Vec<_>>();
                                let Some((source_parameter_position, source_parameter)) =
                                    source_parameters.iter().enumerate().find(|(_, parameter)| {
                                        parameter.symbol == source_path.symbol
                                    })
                                else {
                                    continue;
                                };
                                if bare_dynamic_parameter_trait(
                                    program,
                                    source_parameter.type_reference,
                                ) != Some(target_trait)
                                {
                                    continue;
                                }
                                let mut incoming = transfers
                                    .iter()
                                    .filter(|transfer| {
                                        transfer.target_machine == caller.symbol
                                            && transfer.target_state == caller_state.symbol
                                            && transfer.parameter == source_parameter.symbol
                                            && transfer.target_trait == target_trait
                                    })
                                    .collect::<Vec<_>>();
                                incoming
                                    .sort_by_key(|incoming| incoming.edge().canonical_order_key());
                                let Some(&inbound_call_site_count) =
                                    inbound_call_site_counts.get(&(
                                        caller_state.symbol.arena_index(),
                                        caller_state.symbol.generation(),
                                    ))
                                else {
                                    continue;
                                };
                                if inbound_call_site_count != incoming.len()
                                    || !matches!(incoming.len(), 1 | 2)
                                    || (incoming.len() == 2
                                        && incoming
                                            .iter()
                                            .any(|incoming| incoming.source_paths.len() != 1))
                                    || incoming.iter().any(|incoming| {
                                        !incoming.has_complete_source_custody(&transfers)
                                    })
                                {
                                    continue;
                                }
                                let Ok(source_parameter_position) =
                                    u32::try_from(source_parameter_position)
                                else {
                                    continue;
                                };
                                let source_paths = incoming
                                    .into_iter()
                                    .flat_map(|incoming| incoming.source_paths.clone())
                                    .collect();
                                let Ok(source_predecessor_count) =
                                    u32::try_from(inbound_call_site_count)
                                else {
                                    continue;
                                };
                                (
                                checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                                    parameter_position: source_parameter_position,
                                },
                                source_predecessor_count,
                                source_paths,
                            )
                            };
                        let mut transfer = checked_trees::CheckedDynamicDescriptorTransferPlan {
                            caller_machine: caller.symbol,
                            caller_state: caller_state.symbol,
                            coordinate,
                            target_machine: target_machine.symbol,
                            target_state: target_state.symbol,
                            parameter_position,
                            parameter: parameter.symbol,
                            target_trait,
                            source_binding: source_path.symbol,
                            source,
                            source_predecessor_count,
                            source_paths: Vec::new(),
                        };
                        let edge = transfer.edge();
                        for path in &mut source_paths {
                            path.edges.push(edge.clone());
                        }
                        transfer.source_paths = source_paths;
                        transfers.push(transfer);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    transfers.sort_by_key(|transfer| {
        (
            transfer.caller_machine.arena_index(),
            transfer.caller_machine.generation(),
            transfer.caller_state.arena_index(),
            transfer.caller_state.generation(),
            transfer.coordinate.statement_index,
            transfer.coordinate.call_ordinal,
            transfer.parameter_position,
        )
    });
    transfers
}

/// Independent syntactic roster used to prove that a propagated parameter
/// sees every predecessor. Counting only successfully published descriptor
/// transfers would let an unrecognized third edge disappear from a join.
pub(crate) fn inbound_call_site_counts(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> BTreeMap<(u32, u32), usize> {
    let mut counts = BTreeMap::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let Some(flow) = state_flow(facts, machine.symbol, state.symbol) else {
                continue;
            };
            for call in facts.flow.control.calls.span_or_empty(flow.calls) {
                *counts
                    .entry((
                        call.target_symbol.arena_index(),
                        call.target_symbol.generation(),
                    ))
                    .or_default() += 1;
            }
        }
    }
    counts
}

fn bare_dynamic_parameter_trait(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            bare_dynamic_parameter_trait(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            bare_dynamic_parameter_trait(program, *base_type)
        }
        TypeReferenceNode::DynamicTrait {
            symbol,
            conformance: None,
            ..
        } if symbol.is_valid() => Some(*symbol),
        _ => None,
    }
}
