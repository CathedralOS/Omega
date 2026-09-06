//! Source-bound ordinary Unit roots; shared Unit lowering admits their bodies.

use super::*;

pub(in crate::attached_unit::composed_control) fn retain_call_target<'a>(
    checked: &'a CheckedTrees,
    root: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    plans: &'a checked_trees::CheckedUnitEffectPlans,
    targets: &mut Vec<(&'a checked_trees::CheckedUnitEffectMachinePlan, String)>,
) -> Result<(), LoweringError> {
    crate::call_source_custody::validate_operation(checked, root, state.state, operation)?;
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        structural_arguments,
        claim_transfers,
        ..
    } = operation
    else {
        unreachable!("internal leaf shape was validated")
    };
    if !structural_arguments.is_empty() || !claim_transfers.is_empty() {
        return unsupported("composed internal Unit call requires structural transfer lowering");
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
    if target.state != *target_state
        || target.contract_report_fingerprint != *target_contract_report_fingerprint
        || !checked_unit_target_reach_matches(*service_reach, target.contract_service_reach)
        || !target.structural_parameters.is_empty()
        || !target.entry_claims.is_empty()
    {
        return unsupported("composed internal Unit call disagrees with its checked target");
    }
    let identity = checked_terminal_machine_name(checked, target.machine)?.to_owned();
    if !targets
        .iter()
        .any(|(candidate, _)| candidate.machine == target.machine)
    {
        targets.push((target, identity));
    }
    Ok(())
}
