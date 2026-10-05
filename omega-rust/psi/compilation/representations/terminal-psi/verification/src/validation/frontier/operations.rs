//! What each operation does to the structural ownership frontier.

use super::super::{
    BlockId, ClaimId, ModuleError, OperationKind, OperationResult, PlaceId, StructuralAccess,
    StructuralMultiplicity,
};
use super::{
    FrontierWalk, LiveClaim, RestorationDebt, StructuralOwnershipFrontier,
    projected_root_is_fully_consumed, validate_owned_reads,
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
        ..
    } = *walk;
    super::validate_partial_storage_access(module, machine, operation, frontier)?;
    if let OperationKind::ReleaseReference { source, path } = &operation.kind
        && !path.is_empty()
        && frontier
            .claims
            .values()
            .any(|claim| claim.input == Some(*source))
    {
        return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
            operation: operation.id,
            place: *source,
        });
    }
    // An operation touching an open borrowed-storage window's absent subtree
    // rejects before any custody effect of its own is applied.
    super::super::borrowed_windows::check_operation(walk, operation, frontier)?;
    // A new result may reuse its static place on the next traversal, but it
    // cannot overwrite storage or a carrier still held by a live reference.
    // Check before applying effects, which may publish this result's loans.
    if let Some(result) = operation.result.structural() {
        super::super::references::check_root_access(machine, &frontier.references, result.place)?;
        if frontier.partial_custody_paths.contains_key(&result.place) {
            if !frontier.owned_places.contains_key(&result.place)
                && projected_root_is_fully_consumed(module, machine, frontier, result.place)
            {
                frontier.partial_custody_paths.remove(&result.place);
            } else {
                return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: operation.id,
                    place: result.place,
                });
            }
        }
    }
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
        walk.borrowed_reference_sources,
        &mut frontier.references,
    )?;
    validate_owned_reads(module, machine, operation, frontier)?;
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
    // Quoted bytes denote immutable image storage, not an affine local.
    // Control validation checks their establishment and availability; recording
    // disposal debt here would manufacture an obligation at every loop entry.
    let mut consumed_places = consumed_places(walk, operation);
    consumed_places.extend(leaf_moved_place);
    if let Some(place) =
        walk.shared_loans
            .disturbed(module, machine, block, operation, &consumed_places)
    {
        return Err(ModuleError::SharedStructuralLoanDisturbed {
            operation: operation.id,
            place,
        });
    }
    if let Some(place) =
        walk.descriptor_loans
            .disturbed(module, machine, operation, &consumed_places)
    {
        return Err(ModuleError::BorrowedDescriptorLoanDisturbed {
            operation: operation.id,
            place,
        });
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
        callee,
        structural_arguments,
        claim_transfers,
        ..
    }
    | OperationKind::CallStructuralWithScalarArguments {
        callee,
        structural_arguments,
        claim_transfers,
        ..
    } = &operation.kind
    {
        let callee = machines.get(callee).expect("call target was validated");
        for (argument_index, (argument, parameter)) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
            .enumerate()
        {
            if argument.access != StructuralAccess::Owned {
                continue;
            }
            let mut expected = callee
                .entry_claims
                .iter()
                .filter(|claim| claim.input == parameter.place)
                .map(|claim| (claim.claim, claim.path.as_slice()))
                .collect::<std::collections::BTreeMap<_, _>>();
            for claim in &callee.content_entry_claims {
                if claim.input.root == parameter.place {
                    expected.entry(claim.claim).or_insert(&[]);
                }
            }
            let mut expected_paths = expected.into_values().collect::<Vec<_>>();
            let mut actual_paths = frontier
                .claims
                .iter()
                .filter(|(_, claim)| {
                    claim.input == Some(argument.place) && claim.path.starts_with(&argument.path)
                })
                .map(|(identity, claim)| {
                    if claim_transfers
                        .iter()
                        .filter(|transfer| {
                            transfer.claim == *identity
                                && transfer.argument_index as usize == argument_index
                        })
                        .count()
                        != 1
                    {
                        return Err(ModuleError::UnitCallClaimPresenceMismatch {
                            operation: operation.id,
                            argument_index: argument_index as u32,
                        });
                    }
                    Ok(&claim.path[argument.path.len()..])
                })
                .collect::<Result<Vec<_>, ModuleError>>()?;
            expected_paths.sort();
            actual_paths.sort();
            if expected_paths != actual_paths {
                return Err(ModuleError::UnitCallClaimPresenceMismatch {
                    operation: operation.id,
                    argument_index: argument_index as u32,
                });
            }
        }
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
            if argument.access != StructuralAccess::Owned
                || claim.input != Some(argument.place)
                || !claim.path.starts_with(&argument.path)
            {
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
    let released_argument = match &operation.kind {
        OperationKind::ReleaseReference { source, path } if !path.is_empty() => {
            Some(terminal_psi::StructuralArgument {
                place: *source,
                path: path.clone(),
                access: StructuralAccess::Owned,
            })
        }
        _ => None,
    };
    let mut projected_arguments = projected_arguments(operation);
    projected_arguments.extend(released_argument.as_ref());
    for argument in projected_arguments
        .iter()
        .filter(|argument| !argument.path.is_empty() && argument.access == StructuralAccess::Owned)
    {
        if !frontier.owned_places.contains_key(&argument.place) {
            // A projected owned argument out of a borrowed exclusive root
            // transfers the member out of this machine entirely — the
            // referent's owner receives the hole at return. Record it as a
            // transferred restoration hole: the same absent-storage fact an
            // open window carries, so member access still rejects, double
            // moves still fail, and a matching store may reseat the member.
            let exclusively_borrowed = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == argument.place)
                .or_else(|| super::super::block_views::parameter(machine, argument.place))
                .is_some_and(|parameter| parameter.access == StructuralAccess::MutableBorrow);
            if !exclusively_borrowed {
                return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: operation.id,
                    place: argument.place,
                });
            }
            let (place, region) = super::super::structural::access_regions::argument_region(
                module, machine, argument,
            );
            let Some(leaf_type) =
                super::super::structural::operations::caller_structural_root_type(
                    machine,
                    argument.place,
                )
                .and_then(|root_type| {
                    super::super::foundation::resolve_runtime_projection(
                        module,
                        root_type,
                        &argument.path,
                    )
                })
            else {
                return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: operation.id,
                    place: argument.place,
                });
            };
            for (root, hole) in
                super::super::borrowed_windows::expand_region(walk.window_aliases, place, region)
            {
                let holes = frontier.restoration_debt.entry(root).or_default();
                if holes
                    .keys()
                    .any(|existing| hole.starts_with(existing) || existing.starts_with(&hole))
                {
                    return Err(ModuleError::OverlappingProjectedStructuralMove {
                        operation: operation.id,
                        place: argument.place,
                    });
                }
                holes.insert(
                    hole,
                    RestorationDebt {
                        structural_type: leaf_type,
                        reseat_owed: false,
                    },
                );
            }
            continue;
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
        }
    }
    // Borrowing the backing does not remove affine custody of the descriptor.
    // Its explicit discard closes the carrier; permission to access the backing
    // remains a separate judgment.
    // Plain copy payloads are checked for producer dominance elsewhere,
    // not retained as disposal obligations. Claim-bearing results must
    // still enter this transaction even when their carrier is copyable.
    if let OperationResult::Structural(result) = &operation.result
        && (result.multiplicity != StructuralMultiplicity::Unrestricted
            || super::super::byte_sequence::subslice::borrowed_result_in_module(
                module,
                machine,
                result.place,
            )
            .is_none())
        && super::super::byte_sequence::concatenation::owned_result(machine, result.place).is_none()
        && terminal_semantics::byte_owned_call_result(module, machine, result.place).is_none()
        && (result.multiplicity != StructuralMultiplicity::Unrestricted
            || super::super::element_view::subslice::borrowed_result_in_module(
                module,
                machine,
                result.place,
            )
            .is_none())
        && (result.multiplicity != StructuralMultiplicity::Unrestricted
            || super::super::record::shared_view_member_source(module, machine, result.place)
                .is_none())
        && super::super::primitive_storage::local_result(machine, result.place).is_none()
        && !super::super::scalar::array::plain_return_source(module, machine, result.place)
        && !(result.multiplicity == StructuralMultiplicity::Unrestricted
            && ((super::super::scalar::case::plain_return_source(module, machine, result.place)
                || super::super::structural::case::plain_return_source(
                    module,
                    machine,
                    result.place,
                ))
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
        OperationKind::ReleaseReference { source, path } if path.is_empty() => vec![*source],
        OperationKind::EstablishRecord { .. }
        | OperationKind::EstablishStructuralCase { .. }
        | OperationKind::EstablishStructuralArray { .. } => operation
            .kind
            .constructor_arguments()
            .filter_map(|argument| {
                if !argument.path.is_empty() {
                    return None;
                }
                super::super::structural::result_contracts::source_signature(
                    machine,
                    argument.place,
                )
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
            super::super::structural::result_contracts::source_signature(machine, value.place)
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

/// Structural operands whose projected custody is transferred by this operation.
pub(super) fn projected_arguments(
    operation: &terminal_psi::Operation,
) -> Vec<&terminal_psi::StructuralArgument> {
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
        } => structural_arguments.iter().collect(),
        OperationKind::EstablishRecord { .. }
        | OperationKind::EstablishStructuralCase { .. }
        | OperationKind::EstablishStructuralArray { .. } => operation
            .kind
            .constructor_arguments()
            .filter(|argument| !argument.path.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}
