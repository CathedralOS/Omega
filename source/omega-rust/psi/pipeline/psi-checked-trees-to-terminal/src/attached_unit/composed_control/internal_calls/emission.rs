//! Leaf calls and independently admitted target-machine emission.

use super::*;

pub(in crate::attached_unit::composed_control) fn emit_leaf(
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    block: BlockId,
    targets: &[super::super::catalogs::LoweredComposedInternalTarget],
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
    let target = lookup_target(targets, *target_machine)?;
    let mut operations = OperationBuffer::new(*next_operation - 1);
    emit_call(&mut operations, target.id);
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

pub(in crate::attached_unit::composed_control) fn emit_targets(
    checked: &CheckedTrees,
    targets: &[super::super::catalogs::LoweredComposedInternalTarget],
    type_ids: &[(String, StructuralTypeId)],
    service_ids: &[(ServiceReachId, ServiceId)],
    next_operation: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
) -> Result<Vec<TerminalMachine>, LoweringError> {
    targets
        .iter()
        .map(|target| {
            let block = block_id(allocate_dense(next_block)?);
            let mut operations = OperationBuffer::new(*next_operation - 1);
            if let Some(target_machine) = target.nested_call_target {
                let callee = lookup_target(targets, target_machine)?;
                emit_call(&mut operations, callee.id);
                *next_operation = operations.next_identity;
            }
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
                    operations: operations.operations,
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

fn lookup_target(
    targets: &[super::super::catalogs::LoweredComposedInternalTarget],
    source: psi_symbols::SymbolHandle,
) -> Result<&super::super::catalogs::LoweredComposedInternalTarget, LoweringError> {
    targets
        .iter()
        .find(|target| target.source == source)
        .ok_or(LoweringError::Unsupported(
            "composed internal Unit call target is absent from its exact catalog",
        ))
}

fn emit_call(operations: &mut OperationBuffer, callee: MachineId) {
    let id = operations.allocate();
    operations.push(Operation {
        id,
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
}
