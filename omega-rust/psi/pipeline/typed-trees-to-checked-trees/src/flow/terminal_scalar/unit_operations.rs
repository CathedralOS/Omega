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
                            if exact.next().is_some() {
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
                                } if claim_transfers.is_empty() => Some(operation),
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
