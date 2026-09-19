//! What each operation does to the structural ownership frontier.

use super::super::{
    BlockId, ClaimId, ModuleError, OperationKind, OperationResult, PlaceId, StructuralAccess,
    StructuralMultiplicity,
};
use super::{
    FrontierWalk, LiveClaim, StructuralOwnershipFrontier, projected_root_is_fully_consumed,
    validate_owned_reads,
};

/// What one operation does to the frontier: it applies the operation's
/// reference effects, checks its owned reads, makes a trivial affine local
/// live, retires the places it consumes and the claims it transfers (after
/// checking each transfer names its claim's exact home), opens partial
/// custody for projected arguments, and makes its structural result and
/// result claims live.
pub(super) fn apply_operation(
    walk: &FrontierWalk<'_>,
    block: BlockId,
    operation: &terminal_psi::Operation,
    frontier: &mut StructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    let FrontierWalk {
        module,
        machine,
        machines,
        dominators,
        ..
    } = *walk;
    // An operation touching an open borrowed-storage window's absent subtree
    // rejects before any custody effect of its own is applied.
    super::super::borrowed_windows::check_operation(walk, operation, frontier)?;
    // A projected reference establishment selects its carrier's leaf: the
    // carrier is consumed whole by that move, so detect it before the
    // reference transaction re-homes the leaf under the result.
    let leaf_moved_place = super::super::references::establishment_moves_leaf(
        module,
        machine,
        &frontier.references,
        operation,
    )
    .map(|(place, _)| place);
    super::super::references::apply_operation(
        module,
        machine,
        machines,
        operation,
        &mut frontier.references,
    )?;
    validate_owned_reads(module, machine, operation, frontier, dominators)?;
    if let OperationKind::EstablishTrivialAffineLocal { destination } = operation.kind
        && frontier
            .owned_places
            .insert(destination, StructuralMultiplicity::Affine)
            .is_some()
    {
        return Err(ModuleError::TrivialAffineLocalAlreadyLive {
            operation: operation.id,
            place: destination,
        });
    }
    let mut consumed_places = consumed_places(walk, operation);
    consumed_places.extend(leaf_moved_place);
    // A shared successor loan keeps its referent root stable for the whole
    // duration of the block that bound it: while a joined view observes the
    // root, no operation here may move it, take an exclusive subloan on it,
    // or write through it. Shared observations on the same root stay legal.
    if let Some(pinned) = walk.shared_loans.get(&block) {
        let disturbed = consumed_places
            .iter()
            .copied()
            .chain(
                projected_arguments(operation)
                    .iter()
                    .filter_map(|argument| {
                        (argument.access != StructuralAccess::SharedBorrow)
                            .then_some(argument.place)
                    }),
            )
            .chain(mutation_destinations(operation))
            .find(|place| pinned.contains(place));
        if let Some(place) = disturbed {
            return Err(ModuleError::SharedStructuralLoanDisturbed {
                operation: operation.id,
                place,
            });
        }
    }
    for place in &consumed_places {
        if frontier.partial_custody_paths.contains_key(place) {
            return Err(
                ModuleError::PartiallyMovedStructuralPlaceUsedWholeAtOperation {
                    operation: operation.id,
                    place: *place,
                },
            );
        }
    }
    // Static result metadata binds a claim to one occurrence, not to
    // its current home. Recheck that exact home before transferring the
    // live lineage; an earlier or future result cannot stand in for it.
    if let OperationKind::CallStructural {
        structural_arguments,
        claim_transfers,
        ..
    }
    | OperationKind::CallStructuralWithScalarArguments {
        structural_arguments,
        claim_transfers,
        ..
    } = &operation.kind
    {
        for transfer in claim_transfers {
            let argument = structural_arguments
                .get(transfer.argument_index as usize)
                .ok_or(ModuleError::ClaimActionArgumentOutOfRange {
                    operation: operation.id,
                    argument_index: transfer.argument_index,
                })?;
            let claim = frontier.claims.get(&transfer.claim).ok_or(
                ModuleError::ClaimNotLiveAtOperation {
                    operation: operation.id,
                    claim: transfer.claim,
                },
            )?;
            if claim.input != Some(argument.place) || !claim.path.starts_with(&argument.path) {
                return Err(ModuleError::ClaimActionPlaceMismatch {
                    operation: operation.id,
                    claim: transfer.claim,
                    argument_index: transfer.argument_index,
                });
            }
        }
    }
    let claims = transferred_claims(operation);
    for claim in claims {
        if frontier.claims.remove(&claim).is_none() {
            return Err(ModuleError::ClaimNotLiveAtOperation {
                operation: operation.id,
                claim,
            });
        }
    }
    for place in consumed_places {
        if frontier.owned_places.remove(&place).is_none() {
            return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: operation.id,
                place,
            });
        }
    }
    // Once the consumed places have left the frontier, a window operation
    // opens or closes its restoration debt; the result place installs below
    // through the ordinary structural-result block.
    super::super::borrowed_windows::apply_operation(walk, operation, frontier)?;
    let projected_arguments = projected_arguments(operation);
    for argument in projected_arguments
        .iter()
        .filter(|argument| !argument.path.is_empty() && argument.access == StructuralAccess::Owned)
    {
        if !frontier.owned_places.contains_key(&argument.place) {
            return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: operation.id,
                place: argument.place,
            });
        }
        let moved = frontier
            .partial_custody_paths
            .entry(argument.place)
            .or_default();
        if moved.iter().any(|existing| {
            existing.starts_with(&argument.path) || argument.path.starts_with(existing)
        }) || !moved.insert(argument.path.clone())
        {
            return Err(ModuleError::OverlappingProjectedStructuralMove {
                operation: operation.id,
                place: argument.place,
            });
        }
        if projected_root_is_fully_consumed(module, machine, frontier, argument.place) {
            frontier.owned_places.remove(&argument.place);
            frontier.partial_custody_paths.remove(&argument.place);
        }
    }
    // Plain copy payloads are checked for producer dominance elsewhere,
    // not retained as disposal obligations. Claim-bearing results must
    // still enter this transaction even when their carrier is copyable.
    if let OperationResult::Structural(result) = &operation.result
        && super::super::byte_sequence_subslice::borrowed_result(machine, result.place).is_none()
        && super::super::primitive_storage::local_result(machine, result.place).is_none()
        && !super::super::scalar_array::plain_return_source(module, machine, result.place)
        && !(result.multiplicity == StructuralMultiplicity::Unrestricted
            && (super::super::scalar_case::plain_return_source(module, machine, result.place)
                || super::super::record::plain_return_source(module, machine, result.place)))
    {
        if frontier
            .owned_places
            .insert(result.place, result.multiplicity)
            .is_some()
        {
            return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: operation.id,
                place: result.place,
            });
        }
        for binding in &result.claims {
            if frontier
                .claims
                .insert(
                    binding.claim,
                    LiveClaim {
                        input: Some(result.place),
                        path: binding.path.clone(),
                        multiplicity: Some(if binding.path.is_empty() {
                            result.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    },
                )
                .is_some()
            {
                return Err(ModuleError::ClaimNotLiveAtOperation {
                    operation: operation.id,
                    claim: binding.claim,
                });
            }
        }
    }
    Ok(())
}

/// The places an operation consumes whole: a released reference, the
/// restricted structural fields of a record, and the owned restricted
/// arguments of a call or boundary call.
fn consumed_places(walk: &FrontierWalk<'_>, operation: &terminal_psi::Operation) -> Vec<PlaceId> {
    let FrontierWalk {
        module,
        machine,
        machines,
        ..
    } = *walk;
    match &operation.kind {
        OperationKind::ReleaseReference { source } => vec![*source],
        OperationKind::EstablishRecord { fields } => fields
            .iter()
            .filter_map(|field| {
                let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
                    return None;
                };
                super::super::structural_result_contracts::source_signature(machine, argument.place)
                    .filter(|source| source.multiplicity != StructuralMultiplicity::Unrestricted)
                    .map(|_| argument.place)
            })
            .collect(),

        OperationKind::CallUnit {
            callee,
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            callee,
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            callee,
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            callee,
            structural_arguments,
            ..
        } => structural_arguments
            .iter()
            .zip(&machines[callee].structural_parameters)
            .filter_map(|(argument, parameter)| {
                (argument.path.is_empty()
                    && parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    .then_some(argument.place)
            })
            .collect::<Vec<_>>(),
        OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            ..
        } => {
            let boundary = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .expect("static validation established the boundary target");
            structural_arguments
                .iter()
                .zip(&boundary.structural_parameters)
                .filter_map(|(argument, parameter)| {
                    (argument.path.is_empty()
                        && parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                        .then_some(argument.place)
                })
                .collect()
        }
        // The repair value is consumed exactly once, like an owned call
        // argument; an unrestricted source stays live by its own rule.
        OperationKind::StoreStructuralField { value, .. } => {
            super::super::structural_result_contracts::source_signature(machine, value.place)
                .filter(|source| source.multiplicity != StructuralMultiplicity::Unrestricted)
                .map(|_| value.place)
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
}

/// The claims an operation transfers or settles: a call's claim transfers
/// or a boundary call's completion receipts.
fn transferred_claims(operation: &terminal_psi::Operation) -> Vec<ClaimId> {
    match &operation.kind {
        OperationKind::CallUnit {
            claim_transfers, ..
        }
        | OperationKind::CallStructuralScalar {
            claim_transfers, ..
        }
        | OperationKind::CallStructural {
            claim_transfers, ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            claim_transfers, ..
        } => claim_transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<Vec<_>>(),
        OperationKind::BoundaryCall {
            completion_receipts,
            ..
        } => completion_receipts
            .iter()
            .map(|settlement| settlement.claim)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    }
}

/// The places an operation writes through without moving custody. A store
/// into a pinned loan root would mutate data a bound shared view observes.
fn mutation_destinations(operation: &terminal_psi::Operation) -> Vec<PlaceId> {
    match &operation.kind {
        OperationKind::StructuralScalarFieldStore { destination, .. }
        | OperationKind::StructuralByteSequenceFieldStore { destination, .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { destination, .. }
        | OperationKind::StoreStructuralField { destination, .. }
        | OperationKind::WriteOnlyPrimitiveStore { destination, .. }
        | OperationKind::WriteOnlyIndexedPrimitiveStore { destination, .. }
        | OperationKind::ByteSequenceWrite { destination, .. } => vec![*destination],
        // Extraction mutates through the borrowed root a shared view may
        // observe, so a pinned source rejects like a store's destination.
        OperationKind::MoveStructuralField { source, .. } => vec![*source],
        _ => Vec::new(),
    }
}

/// The structural arguments a call or boundary call passes.
fn projected_arguments(operation: &terminal_psi::Operation) -> &[terminal_psi::StructuralArgument] {
    match &operation.kind {
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.as_slice(),
        _ => &[],
    }
}
