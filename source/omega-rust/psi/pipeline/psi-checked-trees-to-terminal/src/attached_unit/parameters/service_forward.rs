//! Independent raw checked-to-Terminal replay for one routed-Service hop.

use super::*;

pub(super) fn validate(checked: &CheckedTrees) -> Result<(), LoweringError> {
    let plans = &checked.facts.flow.terminal_unit_effects.machines;
    for caller in plans {
        let receipts = caller
            .structural_parameters
            .iter()
            .filter(|parameter| parameter.fused_service_erasure.is_some())
            .collect::<Vec<_>>();
        if receipts.is_empty() {
            continue;
        }
        let forwards = caller
            .operations
            .iter()
            .filter_map(|operation| match operation {
                psi_checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
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
        if forwards.is_empty() {
            continue;
        }
        let [caller_parameter] = receipts.as_slice() else {
            return unsupported(
                "routed Service forwarding requires exactly one caller carrier receipt",
            );
        };
        let [(coordinate, target_machine, target_state, structural_arguments, claim_transfers)] =
            forwards.as_slice()
        else {
            return unsupported("routed Service forwarding requires exactly one internal call");
        };
        if caller.attachment_type_identity.is_some()
            || !caller.scalar_parameters.is_empty()
            || caller.structural_parameters.len() != 1
            || caller.operations.len() != 2
            || !matches!(
                caller.operations.last(),
                Some(psi_checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit { .. })
            )
            || caller_parameter.position != 0
            || caller_parameter.multiplicity != Multiplicity::Affine
            || caller_parameter.access != psi_checked_trees::CheckedStructuralAccess::Owned
            || caller_parameter.qualifications.len() != 1
            || !claim_transfers.is_empty()
        {
            return unsupported("routed Service forwarding widened beyond one free owned hop");
        }
        let [argument] = structural_arguments.as_slice() else {
            return unsupported("routed Service forwarding requires one structural argument");
        };
        if argument.source_parameter_index() != Some(0)
            || !argument.path.is_empty()
            || argument.byte_sequence_literal().is_some()
            || argument.type_identity != caller_parameter.type_identity
            || argument.access != psi_checked_trees::CheckedStructuralAccess::Owned
        {
            return unsupported(
                "routed Service forwarding must move the exact whole owned caller root",
            );
        }
        let targets = plans
            .iter()
            .filter(|candidate| {
                candidate.machine == **target_machine && candidate.state == **target_state
            })
            .collect::<Vec<_>>();
        let [target] = targets.as_slice() else {
            return unsupported("routed Service forwarding has no unique checked terminal helper");
        };
        let [target_parameter] = target.structural_parameters.as_slice() else {
            return unsupported(
                "routed Service forwarding target requires exactly one structural carrier",
            );
        };
        let Some(caller_receipt) = caller_parameter.fused_service_erasure.as_ref() else {
            unreachable!("filtered routed Service caller receipt")
        };
        let Some(target_receipt) = target_parameter.fused_service_erasure.as_ref() else {
            return unsupported("routed Service forwarding target lost its exact receipt");
        };
        if target.attachment_type_identity.is_some()
            || !target.scalar_parameters.is_empty()
            || target_parameter.position != 0
            || target_parameter.type_identity != caller_parameter.type_identity
            || target_parameter.multiplicity != caller_parameter.multiplicity
            || target_parameter.access != caller_parameter.access
            || target_parameter.qualifications != caller_parameter.qualifications
            || target_receipt.carrier_type_identity != caller_receipt.carrier_type_identity
            || target_receipt.requirement != caller_receipt.requirement
            || target_receipt.provider_plan_digest != caller_receipt.provider_plan_digest
        {
            return unsupported(
                "routed Service forwarding target substituted carrier, requirement, domain, or selected plan",
            );
        }
        let Some(requirement) = checked.traits().iter().find(|definition| {
            definition.is_boundary && definition.symbol == caller_receipt.requirement
        }) else {
            return unsupported("routed Service forwarding lost its boundary requirement");
        };
        let requirement_states = checked.trait_machine_signatures(requirement);
        let Some((target_return, target_body)) = target.operations.split_last() else {
            return unsupported("routed Service forwarding target has no checked body");
        };
        if !matches!(
            target_return,
            psi_checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit { .. }
        ) || target_body.is_empty()
            || target_body.iter().any(|operation| {
                !matches!(operation,
                    psi_checked_trees::CheckedUnitEffectOperationPlan::BoundaryCall { target_state, .. }
                        if requirement_states.iter().any(|signature| signature.symbol == *target_state))
            })
        {
            return unsupported(
                "routed Service forwarding target must terminate in direct boundary calls",
            );
        }

        let flow_states = checked
            .facts
            .flow
            .control
            .states
            .iter()
            .map(|(_, state)| state)
            .filter(|state| {
                state.machine_symbol == caller.machine && state.state_symbol == caller.state
            })
            .collect::<Vec<_>>();
        let [flow_state] = flow_states.as_slice() else {
            return unsupported("routed Service forwarding has no unique source flow state");
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
            return unsupported(
                "routed Service forwarding does not rejoin one exact internal source call",
            );
        }
    }
    Ok(())
}
