//! Source-bound Unit roots; shared Unit lowering admits their complete bodies.

use super::*;
use crate::attached_unit::bodies::UnitBody;

pub(in crate::attached_unit::composed_control) fn retain_call_target<'a>(
    checked: &'a CheckedTrees,
    root: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    plans: &'a checked_trees::CheckedUnitEffectPlans,
    targets: &mut Vec<(UnitBody<'a>, String)>,
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
    if !claim_transfers.is_empty()
        || structural_arguments.iter().any(|argument| {
            argument.source_parameter_index().is_none()
                || !argument.path.is_empty()
                || !matches!(
                    argument.access,
                    checked_trees::CheckedStructuralAccess::MutableBorrow
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                )
        })
    {
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
    let target = UnitBody::find(plans, *target_machine)?;
    let entry = target.entry()?;
    if entry.state != *target_state
        || entry.contract_report_fingerprint != *target_contract_report_fingerprint
        || !checked_unit_target_reach_matches(*service_reach, entry.contract_service_reach)
        || entry.structural_parameters.len() != structural_arguments.len()
        || !entry.entry_claims.is_empty()
    {
        return unsupported("composed internal Unit call disagrees with its checked target");
    }
    for (argument, target) in structural_arguments.iter().zip(entry.structural_parameters) {
        let source = argument
            .source_parameter_index()
            .and_then(|position| state.structural_parameters.get(position as usize))
            .ok_or(LoweringError::Unsupported(
                "composed Unit structural source is absent",
            ))?;
        if source.type_identity != target.type_identity
            || argument.access != target.access
            || source.access != target.access
            || source.multiplicity != target.multiplicity
            || !target.qualifications.is_empty()
        {
            return unsupported("composed Unit structural call authority drifted");
        }
    }
    let identity = checked_terminal_machine_name(checked, entry.machine)?.to_owned();
    for (candidate, _) in targets.iter() {
        if candidate.entry()?.machine == entry.machine {
            return Ok(());
        }
    }
    targets.push((target, identity));
    Ok(())
}
