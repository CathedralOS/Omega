//! Independent target-plan and transitive-call admission.

use super::*;

pub(in crate::attached_unit::composed_control) fn retain_leaf_target<'a>(
    checked: &'a CheckedTrees,
    root: psi_symbols::SymbolHandle,
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    plans: &'a psi_checked_trees::CheckedUnitEffectPlans,
    targets: &mut Vec<(&'a psi_checked_trees::CheckedUnitEffectMachinePlan, String)>,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        structural_arguments,
        claim_transfers,
    } = &state.operations[0]
    else {
        unreachable!("internal leaf shape was validated")
    };
    if !structural_arguments.is_empty() || !claim_transfers.is_empty() {
        return unsupported("composed internal Unit call is not parameterless");
    }
    super::super::admission::retain_exact_flow_call(
        checked,
        root,
        state.state,
        *coordinate,
        *target_state,
    )?;
    if *target_machine == root {
        return unsupported("composed internal Unit call is recursive");
    }
    let target = unique_unit_machine(plans, *target_machine)?;
    validate_call_identity(
        *target_state,
        *target_contract_report_fingerprint,
        *service_reach,
        target,
    )?;
    retain_target_closure(checked, root, plans, target, targets, &mut Vec::new())
}

fn retain_target_closure<'a>(
    checked: &'a CheckedTrees,
    root: psi_symbols::SymbolHandle,
    plans: &'a psi_checked_trees::CheckedUnitEffectPlans,
    target: &'a psi_checked_trees::CheckedUnitEffectMachinePlan,
    targets: &mut Vec<(&'a psi_checked_trees::CheckedUnitEffectMachinePlan, String)>,
    active: &mut Vec<psi_symbols::SymbolHandle>,
) -> Result<(), LoweringError> {
    if active.contains(&target.machine) {
        return unsupported("composed internal Unit target closure is recursive");
    }
    if targets
        .iter()
        .any(|(candidate, _)| candidate.machine == target.machine)
    {
        return Ok(());
    }
    let nested_call = validate_target(checked, target)?;
    let identity = checked_terminal_machine_name(checked, target.machine)?.to_owned();
    targets.push((target, identity));
    let Some(CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        structural_arguments,
        claim_transfers,
    }) = nested_call
    else {
        return Ok(());
    };
    if !structural_arguments.is_empty() || !claim_transfers.is_empty() {
        return unsupported("composed internal Unit transitive call is not parameterless");
    }
    super::super::admission::retain_exact_flow_call(
        checked,
        target.machine,
        target.state,
        *coordinate,
        *target_state,
    )?;
    if *target_machine == root {
        return unsupported("composed internal Unit target calls its composed root");
    }
    let nested_target = unique_unit_machine(plans, *target_machine)?;
    validate_call_identity(
        *target_state,
        *target_contract_report_fingerprint,
        *service_reach,
        nested_target,
    )?;
    active.push(target.machine);
    let result = retain_target_closure(checked, root, plans, nested_target, targets, active);
    active.pop();
    result
}

fn validate_target<'a>(
    checked: &CheckedTrees,
    target: &'a psi_checked_trees::CheckedUnitEffectMachinePlan,
) -> Result<Option<&'a CheckedUnitEffectOperationPlan>, LoweringError> {
    if !target.structural_parameters.is_empty()
        || !target.provider_attachment_requirements.is_empty()
        || !target.trivial_affine_locals.is_empty()
        || !target.entry_claims.is_empty()
        || !target.body_qualifications.is_empty()
    {
        return unsupported("composed internal Unit target escaped source-free custody");
    }
    let nested_call = match target.operations.as_slice() {
        [
            CheckedUnitEffectOperationPlan::ReturnUnit {
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
                ..
            },
        ] if trivial_affine_local_discard_ordinals.is_empty()
            && trivial_affine_discards.is_empty() =>
        {
            None
        }
        [
            call @ CheckedUnitEffectOperationPlan::CallUnit { .. },
            CheckedUnitEffectOperationPlan::ReturnUnit {
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
                ..
            },
        ] if trivial_affine_local_discard_ordinals.is_empty()
            && trivial_affine_discards.is_empty() =>
        {
            Some(call)
        }
        _ => {
            return unsupported(
                "composed internal Unit target is outside the exact empty-or-one-call slice",
            );
        }
    };
    validate_unit_operation_sequence(target)?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(target.machine)
        .ok_or(LoweringError::Unsupported(
            "composed internal Unit target has no checked contract",
        ))?;
    if target.contract_report_fingerprint == 0
        || target.contract_report_fingerprint != contract.report_fingerprint
        || target.contract_commitment != contract.commitment
        || !contract.crash.published().is_empty()
    {
        return unsupported("composed internal Unit target contract drifted after checking");
    }
    Ok(nested_call)
}

fn validate_call_identity(
    target_state: psi_symbols::SymbolHandle,
    target_contract_report_fingerprint: u64,
    service_reach: ServiceReachSummary,
    target: &psi_checked_trees::CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    if target.state != target_state
        || target.contract_report_fingerprint != target_contract_report_fingerprint
        || !checked_unit_target_reach_matches(service_reach, target.contract_service_reach)
    {
        return unsupported(
            "composed internal Unit call does not match its checked target and reach",
        );
    }
    Ok(())
}
