//! Complete ordinary Unit calls after the source graph and ownership ledger exist.

use checked_trees::{CheckFacts, CheckedUnitEffectOperationPlan};
use typed_trees::{TypedTrees, statement::StatementNode};

pub(crate) fn finalize(program: &TypedTrees, facts: &mut CheckFacts) {
    // Borrow the established graph catalog during call construction. It remains
    // available to shared call admission; publish all completed rows afterward.
    let operations = facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .map(|graph| {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == graph.machine)?;
            graph
                .states
                .iter()
                .map(|state| {
                    let source = program
                        .machine_states(machine)
                        .iter()
                        .find(|source| source.symbol == state.state)?;
                    let flow = facts
                        .flow
                        .control
                        .states
                        .iter()
                        .map(|(_, state)| state)
                        .find(|flow| {
                            flow.machine_symbol == graph.machine && flow.state_symbol == state.state
                        })?;
                    let calls = facts.flow.control.calls.span(flow.calls)?;
                    program
                        .statement_table
                        .statements(source.statement_nodes)
                        .iter()
                        .enumerate()
                        .filter(|(_, statement)| matches!(statement, StatementNode::Call(_)))
                        .map(|(ordinal, _)| {
                            let mut exact = calls.iter().filter(|call| {
                                call.statement_index == ordinal && call.call_ordinal == 0
                            });
                            let call = exact.next()?;
                            if exact.next().is_some() || call.has_receiver {
                                return None;
                            }
                            let operation =
                                crate::flow::terminal_unit::calls::build_call_operation(
                                    program,
                                    facts,
                                    machine,
                                    source,
                                    &state.structural_parameters,
                                    &[],
                                    &[],
                                    &[],
                                    call,
                                    true,
                                    None,
                                    &[],
                                )?;
                            match &operation {
                                CheckedUnitEffectOperationPlan::CallUnit {
                                    claim_transfers,
                                    ..
                                } if claim_transfers.is_empty()
                                    && eligible(program, facts, &operation) =>
                                {
                                    Some(operation)
                                }
                                _ => None,
                            }
                        })
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Vec<_>>();
    let mut completed = operations.into_iter();
    facts
        .flow
        .terminal_scalar_graphs
        .machines
        .retain_mut(|graph| {
            let Some(Some(operations)) = completed.next() else {
                return false;
            };
            for (state, operations) in graph.states.iter_mut().zip(operations) {
                state.unit_operations = operations;
            }
            true
        });
}

// The scalar emitter has no contract-substitution or service machinery. Keep
// those semantic operations with the ordinary Unit body owner, before graph
// selection becomes authoritative. The receiver independently checks custody.
fn eligible(
    program: &TypedTrees,
    facts: &CheckFacts,
    operation: &CheckedUnitEffectOperationPlan,
) -> bool {
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Source;
    use typed_trees::types::TypeReferenceNode;
    let CheckedUnitEffectOperationPlan::CallUnit {
        target_machine,
        target_state,
        service_reach,
        structural_arguments,
        ..
    } = operation
    else {
        return false;
    };
    let Some(state) = crate::find_state(program, *target_state) else {
        return false;
    };
    let Some(contract) = facts.contract_plans.for_machine(*target_machine) else {
        return false;
    };
    let plain_primitive = |reference| {
        matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. }
        ) && program.primitive_type_reference(reference).is_some()
    };
    program.state_contracts(state).is_empty()
        && contract.closed_scalar_values.requires().is_empty()
        && contract.closed_scalar_values.ensures().is_empty()
        && contract.crash.published().is_empty()
        && contract
            .crash
            .structural_runtime_requirements()
            .is_none_or(|requirements| requirements.is_empty())
        && facts
            .service_reaches
            .rows
            .services(service_reach.direct)
            .is_empty()
        && facts
            .service_reaches
            .rows
            .services(service_reach.transitive)
            .is_empty()
        && program.state_parameters(state).iter().all(|parameter| {
            !parameter.is_self
                && !parameter.is_const
                && (plain_primitive(parameter.type_reference)
                    || matches!(
                        program.type_reference_table.type_reference(parameter.type_reference),
                        TypeReferenceNode::Reference { referee, .. } if plain_primitive(*referee)
                    ))
        })
        && structural_arguments.iter().all(|argument| {
            argument.path.is_empty()
                && matches!(
                    argument.source,
                    Source::Parameter { .. } | Source::PrimitiveLocal { .. }
                )
        })
}
