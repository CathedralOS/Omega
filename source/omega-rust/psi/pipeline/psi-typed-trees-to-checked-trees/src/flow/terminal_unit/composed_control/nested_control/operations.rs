//! Exact empty-custody operation prefix for a conditional control state.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    match program.statement_table.statements(state.statement_nodes) {
        [StatementNode::Transition(_), StatementNode::Transition(_)] => Some(Vec::new()),
        [
            StatementNode::Call(_),
            StatementNode::Transition(_),
            StatementNode::Transition(_),
        ] => {
            let flow = state_flow(facts, machine.symbol, state.symbol)?;
            let matching_calls = facts
                .flow
                .control
                .calls
                .span_or_empty(flow.calls)
                .iter()
                .filter(|call| call.statement_index == 0 && call.call_ordinal == 0)
                .collect::<Vec<_>>();
            let [call] = matching_calls.as_slice() else {
                return None;
            };
            let operation =
                build_call_operation(program, facts, machine, state, &[], &[], call, false, None)?;
            matches!(
                &operation,
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    claim_transfers,
                    ..
                } if structural_arguments.is_empty() && claim_transfers.is_empty()
            )
            .then_some(vec![operation])
        }
        _ => None,
    }
}
