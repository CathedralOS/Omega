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
    let mut targets = facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .iter()
        .filter(|plan| plan.machine == *target_machine);
    let Some(plan) = targets.next() else {
        return false;
    };
    if targets.next().is_some() || plan.state != *target_state {
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
    // Ordinary ScalarCall cannot transfer structural places or entry claims.
    // Its dense scalar roster must cover the exact authored signature, without
    // erasing a receiver or interpreting source positions as argument ordinals.
    machine.supply_mode == MachineSupplyMode::CheckedBody
        && state.symbol == *target_state
        && program.state_parameters(state).len() == plan.scalar_parameters.len()
        && scalar_arguments.len() == plan.scalar_parameters.len()
        && program
            .state_parameters(state)
            .iter()
            .zip(&plan.scalar_parameters)
            .enumerate()
            .all(|(position, (source, parameter))| {
                !source.is_self
                    && !source.is_const
                    && !source.is_mutable
                    && usize::try_from(parameter.source_position).ok() == Some(position)
                    && program.primitive_type_reference(source.type_reference)
                        == Some(parameter.primitive_type)
            })
        && scalar_arguments
            .iter()
            .zip(&plan.scalar_parameters)
            .all(|(argument, parameter)| {
                let primitive_type = match argument {
                    checked_trees::CheckedCallScalarArgument::Pure(expression) => {
                        crate::values::scalar_expression_type(expression)
                    }
                    checked_trees::CheckedCallScalarArgument::Computation(root) => {
                        let computations = &facts.values.scalar_computations;
                        computations
                            .nodes
                            .is_valid(*root)
                            .then(|| computations.nodes.get(*root).primitive_type)
                    }
                };
                primitive_type == Some(parameter.primitive_type)
            })
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
