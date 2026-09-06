//! Ordinary scalar calls retain an existing body plan, never a fabricated graph.

use super::*;

pub(super) fn is_available(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
) -> bool {
    let CheckedUnitEffectOperationPlan::ScalarCall {
        result,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        target_contract_commitment,
        scalar_arguments,
        structural_arguments,
        claim_transfers,
        coordinate,
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
        return structural_arguments.is_empty() && claim_transfers.is_empty();
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
    let mut shapes = ShapeCollector::new(program);
    let binders = machine_binders(program, machine);
    let signature = if plan.scalar_parameters.is_empty() {
        structural_signature(program, &mut shapes, machine, state, &binders, false)
            .map(|(attachment, structural)| (attachment, structural, Vec::new()))
    } else {
        structural_scalar_signature(program, &mut shapes, machine, state, &binders, false)
    };
    let Some((attachment, structural, scalar)) = signature else {
        return false;
    };
    if attachment != plan.attachment_type_identity
        || structural != plan.structural_parameters
        || scalar != plan.scalar_parameters
        || structural_arguments.len() != structural.len()
        || structural_arguments
            .iter()
            .zip(&structural)
            .any(|(argument, parameter)| {
                argument.type_identity != parameter.type_identity
                    || argument.access != parameter.access
            })
        || claim_transfers.len() != plan.entry_claims.len()
        || claim_transfers
            .iter()
            .zip(&plan.entry_claims)
            .any(|(transfer, claim)| {
                transfer.argument_index != claim.parameter_index
                    || claim.carry != CarryPolicy::STRICT
            })
    {
        return false;
    }
    if entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural,
        program.state_parameters(state),
    )
    .as_ref()
        != Some(&plan.entry_claims)
    {
        return false;
    }
    let Some(flow) = state_flow(facts, caller.machine, caller.state) else {
        return false;
    };
    let mut calls = facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .filter(|call| {
            u32::try_from(call.statement_index).ok() == Some(coordinate.statement_index)
                && u32::try_from(call.call_ordinal).ok() == Some(coordinate.call_ordinal)
        });
    let Some(call) = calls.next() else {
        return false;
    };
    if calls.next().is_some()
        || call_claim_transfers(
            facts,
            caller.machine,
            caller.state,
            call,
            &caller.structural_parameters,
            &caller.entry_claims,
            structural_arguments,
            PermissionEventKind::Transfer,
        )
        .as_ref()
            != Some(claim_transfers)
    {
        return false;
    }
    // Authored positions partition into dense scalar and structural namespaces;
    // no receiver or claim may disappear while selecting the real callee body.
    machine.supply_mode == MachineSupplyMode::CheckedBody
        && state.symbol == *target_state
        && program.state_parameters(state).len()
            == plan.scalar_parameters.len() + plan.structural_parameters.len()
        && scalar_arguments.len() == plan.scalar_parameters.len()
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
        && plan.result_type == result.primitive_type
        && program.primitive_type_reference(state.return_type) == Some(plan.result_type)
        && contract.report_fingerprint == *target_contract_report_fingerprint
        && contract.commitment == *target_contract_commitment
}
