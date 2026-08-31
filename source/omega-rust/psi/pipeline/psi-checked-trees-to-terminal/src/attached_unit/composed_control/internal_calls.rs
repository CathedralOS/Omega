//! Exact terminal targets for parameterless internal Unit-call leaves.

use super::*;

pub(super) fn retain_leaf_target<'a>(
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
    super::admission::retain_exact_flow_call(
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
    {
        return unsupported(
            "composed internal Unit call does not match its checked target and reach",
        );
    }
    validate_target(checked, target)?;
    let identity = checked_terminal_machine_name(checked, target.machine)?.to_owned();
    if !targets
        .iter()
        .any(|(candidate, _)| candidate.machine == target.machine)
    {
        targets.push((target, identity));
    }
    Ok(())
}

fn validate_target(
    checked: &CheckedTrees,
    target: &psi_checked_trees::CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    if !target.structural_parameters.is_empty()
        || !target.provider_attachment_requirements.is_empty()
        || !target.trivial_affine_locals.is_empty()
        || !target.entry_claims.is_empty()
        || !target.body_qualifications.is_empty()
        || !matches!(target.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::ReturnUnit {
                trivial_affine_local_discard_ordinals,
                trivial_affine_discards,
                ..
            }] if trivial_affine_local_discard_ordinals.is_empty()
                && trivial_affine_discards.is_empty())
    {
        return unsupported("composed internal Unit target is outside the exact empty-body slice");
    }
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
    Ok(())
}

pub(super) fn emit_leaf(
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    block: BlockId,
    targets: &[super::catalogs::LoweredComposedInternalTarget],
    next_operation: &mut u64,
    next_edge: &mut u64,
) -> Result<(Block, Vec<LoweredSourceCallOccurrence>), LoweringError> {
    let CheckedUnitEffectOperationPlan::CallUnit {
        target_machine,
        structural_arguments,
        claim_transfers,
        ..
    } = &state.operations[0]
    else {
        unreachable!("admission retained one internal Unit call")
    };
    if !structural_arguments.is_empty() || !claim_transfers.is_empty() {
        return unsupported("composed internal Unit call custody drifted before emission");
    }
    let target = targets
        .iter()
        .find(|target| target.source == *target_machine)
        .ok_or(LoweringError::Unsupported(
            "composed internal Unit call target is absent from its exact catalog",
        ))?;
    let mut operations = OperationBuffer::new(*next_operation - 1);
    let id = operations.allocate();
    operations.push(Operation {
        id,
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: target.id,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    *next_operation = operations.next_identity;
    Ok((
        Block {
            id: block,
            parameters: Vec::new(),
            operations: operations.operations,
            terminator: Terminator::ReturnUnit {
                edge: edge_id(allocate_dense(next_edge)?),
                trivial_affine_discards: Vec::new(),
            },
        },
        Vec::new(),
    ))
}

pub(super) fn emit_targets(
    checked: &CheckedTrees,
    targets: &[super::catalogs::LoweredComposedInternalTarget],
    type_ids: &[(String, StructuralTypeId)],
    service_ids: &[(ServiceReachId, ServiceId)],
    next_edge: &mut u64,
) -> Result<Vec<TerminalMachine>, LoweringError> {
    targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            let block = block_id(
                u64::try_from(index)
                    .map_err(|_| {
                        LoweringError::Unsupported("composed Unit internal block count exceeds u64")
                    })?
                    .checked_add(4)
                    .ok_or(LoweringError::Unsupported(
                        "composed Unit internal block identity space is exhausted",
                    ))?,
            );
            Ok(TerminalMachine {
                id: target.id,
                attachment: Some(lookup_type_id(type_ids, &target.attachment_type_identity)?),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: lower_installation_machine_service_ceiling(
                    checked,
                    target.source,
                    target.contract_service_reach,
                    target.service_reach,
                    service_ids,
                )?,
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block,
                blocks: vec![Block {
                    id: block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(allocate_dense(next_edge)?),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: contract_id(target.id.get()),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            })
        })
        .collect()
}
