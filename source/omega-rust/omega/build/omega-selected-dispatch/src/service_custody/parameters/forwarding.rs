//! Final selected-provider custody replay for one routed-Service hop.

use super::*;

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    parameter: &CheckedUnitStructuralParameterPlan,
    receipt: &psi_checked_trees::CheckedFusedServiceParameterReceipt,
    plan: &psi_checked_trees::CheckedUnitEffectMachinePlan,
) -> Result<(), &'static str> {
    if machine.attached_data.is_some()
        || plan.attachment_type_identity.is_some()
        || !plan.scalar_parameters.is_empty()
        || plan.structural_parameters.len() != 1
        || parameter.position != 0
        || parameter.multiplicity != psi_language_semantics::Multiplicity::Affine
        || parameter.access != psi_checked_trees::CheckedStructuralAccess::Owned
        || parameter.qualifications.len() != 1
        || plan.operations.len() != 2
        || !matches!(
            plan.operations.last(),
            Some(CheckedUnitEffectOperationPlan::ReturnUnit { .. })
        )
    {
        return Err("the caller widened beyond one free owned Service hop");
    }
    let forwards = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                claim_transfers,
                ..
            } => Some((
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                claim_transfers,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(coordinate, target_machine, target_state, structural_arguments, claim_transfers)] =
        forwards.as_slice()
    else {
        return Err("the caller does not retain exactly one internal call");
    };
    let [argument] = structural_arguments.as_slice() else {
        return Err("the forwarding edge does not retain exactly one argument");
    };
    if !claim_transfers.is_empty()
        || argument.source_parameter_index() != Some(0)
        || !argument.path.is_empty()
        || argument.byte_sequence_literal().is_some()
        || argument.type_identity != parameter.type_identity
        || argument.access != psi_checked_trees::CheckedStructuralAccess::Owned
    {
        return Err("the forwarding edge is not one whole-root owned move");
    }
    let targets = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .filter(|candidate| {
            candidate.machine == **target_machine && candidate.state == **target_state
        })
        .collect::<Vec<_>>();
    let [target] = targets.as_slice() else {
        return Err("the forwarding edge has no unique checked target plan");
    };
    let [target_parameter] = target.structural_parameters.as_slice() else {
        return Err("the forwarding target does not retain one exact carrier");
    };
    let Some(target_receipt) = target_parameter.fused_service_erasure.as_ref() else {
        return Err("the forwarding target lost its routed Service receipt");
    };
    if target.attachment_type_identity.is_some()
        || !target.scalar_parameters.is_empty()
        || target_parameter.position != 0
        || target_parameter.type_identity != parameter.type_identity
        || target_parameter.multiplicity != parameter.multiplicity
        || target_parameter.access != parameter.access
        || target_parameter.qualifications != parameter.qualifications
        || target_receipt.carrier_type_identity != receipt.carrier_type_identity
        || target_receipt.requirement != receipt.requirement
        || target_receipt.provider_plan_digest != receipt.provider_plan_digest
    {
        return Err("the forwarding target substituted carrier, domain, requirement, or plan");
    }
    let Some(requirement) = checked
        .traits()
        .iter()
        .find(|definition| definition.is_boundary && definition.symbol == receipt.requirement)
    else {
        return Err("the forwarding route lost its boundary requirement");
    };
    let requirement_states = checked.trait_machine_signatures(requirement);
    let Some((target_return, target_body)) = target.operations.split_last() else {
        return Err("the forwarding target has no checked body");
    };
    if !matches!(
        target_return,
        CheckedUnitEffectOperationPlan::ReturnUnit { .. }
    ) || target_body.is_empty()
        || target_body.iter().any(|operation| {
            !matches!(operation,
                CheckedUnitEffectOperationPlan::BoundaryCall { target_state, .. }
                    if requirement_states.iter().any(|signature| signature.symbol == *target_state))
        })
    {
        return Err("the forwarding target does not terminate in direct requirement calls");
    }

    let flow_states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, flow)| flow)
        .filter(|flow| flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol)
        .collect::<Vec<_>>();
    let [flow_state] = flow_states.as_slice() else {
        return Err("the forwarding caller has no unique flow state");
    };
    let source_calls = checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(flow_state.calls);
    if !matches!(source_calls, [source]
        if source.statement_index == usize::try_from(coordinate.statement_index).unwrap_or(usize::MAX)
            && source.call_ordinal == usize::try_from(coordinate.call_ordinal).unwrap_or(usize::MAX)
            && source.target_symbol == **target_state
            && !source.has_receiver)
    {
        return Err("the forwarding plan does not rejoin one exact internal source call");
    }
    Ok(())
}
