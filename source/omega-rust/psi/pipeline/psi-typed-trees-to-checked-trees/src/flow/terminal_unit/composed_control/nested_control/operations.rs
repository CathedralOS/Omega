//! Exact empty-custody operation prefix for a conditional control state.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    boundaries: &[CheckedBoundaryMachinePlan],
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let (prefix_count, _, _) = super::topology::conditional_parts(
        program.statement_table.statements(state.statement_nodes),
    )?;
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span_or_empty(flow.calls);
    (0..prefix_count)
        .map(|statement_index| {
            let source_index = usize::try_from(statement_index).ok()?;
            let matching_calls = calls
                .iter()
                .filter(|call| call.statement_index == source_index && call.call_ordinal == 0)
                .collect::<Vec<_>>();
            let [call] = matching_calls.as_slice() else {
                return None;
            };
            let operation = build_call_operation(
                program,
                facts,
                machine,
                state,
                &[],
                &[],
                &[],
                call,
                false,
                None,
            )?;
            let admitted = match &operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    claim_transfers,
                    ..
                } => structural_arguments.is_empty() && claim_transfers.is_empty(),
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    target_machine,
                    scalar_arguments,
                    structural_arguments,
                    completion_receipts,
                    ..
                } => {
                    let matching = boundaries
                        .iter()
                        .filter(|boundary| boundary.machine == *target_machine)
                        .collect::<Vec<_>>();
                    matches!(matching.as_slice(), [boundary]
                        if scalar_arguments.is_empty()
                            && structural_arguments.is_empty()
                            && completion_receipts.is_empty()
                            && boundary.scalar_parameters.is_empty()
                            && boundary.structural_parameters.is_empty()
                            && boundary.domain_requirements.is_empty()
                            && boundary.result_type.is_none())
                }
                _ => false,
            };
            admitted.then_some(operation)
        })
        .collect()
}
