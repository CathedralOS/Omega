//! One exact boundary effect followed by Unit return.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    boundaries: &[CheckedBoundaryMachinePlan],
) -> Option<CheckedComposedUnitControlStatePlan> {
    let [StatementNode::Call(_)] = program.statement_table.statements(state.statement_nodes) else {
        return None;
    };
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let [call] = facts.flow.control.calls.span_or_empty(flow.calls) else {
        return None;
    };
    if call.statement_index != 0 || call.call_ordinal != 0 {
        return None;
    }
    let operation =
        build_call_operation(program, facts, machine, state, &[], &[], call, false, None)?;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        target_machine,
        structural_arguments,
        completion_receipts,
        ..
    } = &operation
    else {
        return None;
    };
    if !structural_arguments.is_empty()
        || !completion_receipts.is_empty()
        || !boundaries.iter().any(|plan| {
            plan.machine == *target_machine
                && plan.structural_parameters.is_empty()
                && plan.domain_requirements.is_empty()
                && plan.result_type.is_none()
        })
    {
        return None;
    }
    Some(CheckedComposedUnitControlStatePlan {
        state: state.symbol,
        structural_parameters: Vec::new(),
        scalar_parameters: Vec::new(),
        entry_claims: Vec::new(),
        operations: vec![operation],
        terminator: CheckedComposedUnitControlTerminatorPlan::ReturnUnit,
    })
}
