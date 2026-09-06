//! Ordinary scalar calls retain an existing body plan, never a fabricated graph.

use super::*;

pub(super) fn is_available(
    program: &TypedTrees,
    facts: &CheckFacts,
    operation: &CheckedUnitEffectOperationPlan,
) -> bool {
    let CheckedUnitEffectOperationPlan::ScalarCall {
        result,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        target_contract_commitment,
        scalar_arguments,
        ..
    } = operation
    else {
        return false;
    };
    if facts
        .flow
        .terminal_scalar_graphs
        .for_machine(*target_machine)
        .is_some()
    {
        return true;
    }
    let Some(plan) = facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .iter()
        .find(|plan| plan.machine == *target_machine && plan.state == *target_state)
    else {
        return false;
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == *target_machine)
    else {
        return false;
    };
    let [state] = program.machine_states(machine) else {
        return false;
    };
    let Some(contract) = facts.contract_plans.for_machine(*target_machine) else {
        return false;
    };
    // This return-plan family currently has no scalar entry parameters.
    // Ordinary ScalarCall cannot transfer structural places or entry claims.
    // Requiring the exact empty signature also excludes an erased receiver.
    machine.supply_mode == MachineSupplyMode::CheckedBody
        && state.symbol == *target_state
        && program.state_parameters(state).is_empty()
        && scalar_arguments.is_empty()
        && plan.structural_parameters.is_empty()
        && plan.entry_claims.is_empty()
        && plan.result_type == result.primitive_type
        && program.primitive_type_reference(state.return_type) == Some(plan.result_type)
        && contract.report_fingerprint == *target_contract_report_fingerprint
        && contract.commitment == *target_contract_commitment
        && matches!(&plan.boundary_call,
            CheckedUnitEffectOperationPlan::BoundaryCall { structural_arguments, completion_receipts, .. }
                if structural_arguments.is_empty() && completion_receipts.is_empty())
}
