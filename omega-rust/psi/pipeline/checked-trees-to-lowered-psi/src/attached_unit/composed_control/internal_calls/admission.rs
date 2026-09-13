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
    crate::call_source_custody::validate_operation(
        checked,
        root,
        state.state,
        operation,
        &state.structural_parameters,
    )?;
    crate::attached_unit::structural_calls::validate_custody(
        checked,
        root,
        state.state,
        operation,
    )?;
    let (
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        structural_arguments,
    ) = match operation {
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            structural_arguments,
            claim_transfers,
            ..
        } if claim_transfers.is_empty() => (
            coordinate,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            service_reach,
            structural_arguments,
        ),
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            target_contract_commitment,
            service_reach,
            structural_arguments,
            result,
            custody,
            ..
        } => {
            let target = UnitBody::find(plans, *target_machine)?;
            let checked_trees::CheckedControlResultPlan::Structural(signature) = target.result()?
            else {
                return unsupported("internal structural call has no structural graph result");
            };
            let contract = checked
                .facts
                .contract_plans
                .for_machine(*target_machine)
                .ok_or(LoweringError::Unsupported(
                    "internal structural call contract missing",
                ))?;
            if target_contract_commitment.is_zero()
                || *target_contract_commitment != contract.commitment
                || result.type_identity != signature.type_identity
                || result.multiplicity != signature.multiplicity
                || signature.qualifications != custody.result_qualifications
                || result.statement_index != coordinate.statement_index
            {
                return unsupported("internal structural call result or commitment drifted");
            }
            (
                coordinate,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                service_reach,
                structural_arguments,
            )
        }
        _ => return unsupported("internal call has unsupported claim transfers or result"),
    };
    if !matches!(
        operation,
        CheckedUnitEffectOperationPlan::StructuralCall { .. }
    ) && structural_arguments.iter().any(|argument| {
        (argument.source_parameter_index().is_none() && argument.byte_sequence_literal().is_none())
            || !matches!(
                argument.access,
                checked_trees::CheckedStructuralAccess::MutableBorrow
                    | checked_trees::CheckedStructuralAccess::SharedBorrow
            )
    }) {
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
    if matches!(operation, CheckedUnitEffectOperationPlan::CallUnit { .. })
        && target.result()? != checked_trees::CheckedControlResultPlan::Unit
    {
        return unsupported("internal Unit call cannot erase a structural result");
    }
    let entry = target.entry()?;
    if entry.state != *target_state
        || entry.contract_report_fingerprint != *target_contract_report_fingerprint
        || !checked_unit_target_reach_matches(*service_reach, entry.contract_service_reach)
        || entry.structural_parameters.len() != structural_arguments.len()
        || (!matches!(
            operation,
            CheckedUnitEffectOperationPlan::StructuralCall { .. }
        ) && !entry.entry_claims.is_empty())
    {
        return unsupported("composed internal Unit call disagrees with its checked target");
    }
    for (argument, target) in structural_arguments.iter().zip(entry.structural_parameters) {
        if argument.byte_sequence_literal().is_some() {
            if argument.type_identity != target.type_identity
                || argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
                || argument.access != target.access
                || target.multiplicity != Multiplicity::Unrestricted
                || !target.qualifications.is_empty()
                || !plans.structural_types.iter().any(|declaration| {
                    declaration.identity == target.type_identity
                        && matches!(
                            declaration.shape,
                            checked_trees::CheckedUnitStructuralTypeShape::ByteSequence(
                                checked_trees::CheckedByteSequenceCarrier::BorrowedView
                            )
                        )
                })
            {
                return unsupported(
                    "composed Unit literal call lost its exact shared-view custody",
                );
            }
            continue;
        }
        let source = argument
            .source_parameter_index()
            .and_then(|position| state.structural_parameters.get(position as usize))
            .ok_or(LoweringError::Unsupported(
                "composed Unit structural source is absent",
            ))?;
        // A projected argument names its owner's parameter, not a second
        // parameter of the referent type. Source custody above checks the
        // authored path; common transfer-shape validation before emission
        // checks the projected type and byte-view presentation independently.
        if argument.access != target.access
            || source.access != target.access
            || (argument.path.is_empty()
                && (source.type_identity != target.type_identity
                    || source.multiplicity != target.multiplicity))
            || source.qualifications != target.qualifications
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
