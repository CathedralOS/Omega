use crate::OptimizationUnitValidationError;
use crate::current_ownership::cleanup::{
    validate_scalar_cleanup_actions, validate_unit_cleanup_actions,
};
use crate::current_ownership::mutations::{
    apply_edge_trivial_affine_discards, expected_trivial_affine_discards, insert_owned_result,
    reject_live_linear_claim,
};
use crate::current_ownership::structural::projected_root_is_fully_consumed;
use crate::current_ownership::{
    CurrentOwnership, LiveClaim, apply_edge_partial_affine_discards, bind_owned_parameters,
};
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, MachineId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration,
};

/// Byte observations borrow storage without consuming it. Dominating producer
/// evidence does not make a consumed or partially moved owner readable again.
fn validate_byte_observations(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    operation: &O,
    frontier: &CurrentOwnership,
) -> Result<(), OptimizationUnitValidationError> {
    let observe = |source| {
        let invalid = || OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
            machine: function.machine,
            block,
            node,
            place: source,
        };
        let signature = crate::unit_validation::operation_contracts::structural_source_contract(
            function, source,
        )
        .ok_or_else(invalid)?;
        if signature.access == StructuralAccess::Owned
            && ((signature.multiplicity != StructuralMultiplicity::Unrestricted
                && frontier.owned_places.get(&source) != Some(&signature.multiplicity))
                || frontier.partial_custody_paths.contains_key(&source))
        {
            return Err(invalid());
        }
        Ok(())
    };
    match operation {
        O::StructuralByteSequenceFieldLength { source, .. }
        | O::StructuralByteSequenceFieldRead { source, .. } => observe(*source),
        O::EstablishByteSequenceConcatenation { left, right, .. } => {
            observe(left.source())?;
            observe(right.source())
        }
        _ => Ok(()),
    }
}

/// Coherence identifies the resident, but grants no access to its storage.
/// Rejoin the current root contract and custody before observing or modifying it.
fn validate_atomic_access(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    operation: &O,
    frontier: &CurrentOwnership,
) -> Result<(), OptimizationUnitValidationError> {
    let O::AtomicEvent { event, .. } = operation else {
        return Ok(());
    };
    let Some(location) = event.location() else {
        return Ok(());
    };
    let invalid = || OptimizationUnitValidationError::CurrentReferenceCustodyViolation {
        machine: function.machine,
        block,
        node: Some(node),
        reason: "atomic event has no compatible current storage authority",
    };
    // Coherence transports whole arrival bindings. An operation-result alias
    // needs its own residency transfer before it can name an atomic location.
    let signature = function
        .arrival_parameter(location.root)
        .ok_or_else(invalid)?;
    let carrier = crate::unit_validation::structural_catalog::resolve_structural_path(
        structural_types,
        signature.structural_type,
        &location.path,
    )
    .ok_or_else(invalid)?;
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &structural_types.get(&carrier).ok_or_else(invalid)?.shape
    else {
        return Err(invalid());
    };
    // Shared access grants atomic operations only on a declared atomic cell,
    // never an ordinary same-width scalar. Observation still needs read access.
    if !fields.iter().any(|field| {
        field.id == location.field
            && !field.relevance.is_erased()
            && matches!(
                field.field_type,
                terminal_psi::StructuralFieldType::Atomic(_)
            )
    }) || !signature.qualifications.is_empty()
        || !signature.projected_qualifications.is_empty()
        || (event.observes_resident() && signature.access == StructuralAccess::WriteOnlyBorrow)
    {
        return Err(invalid());
    }
    if signature.access == StructuralAccess::Owned
        && ((signature.multiplicity != StructuralMultiplicity::Unrestricted
            && frontier.owned_places.get(&location.root) != Some(&signature.multiplicity))
            || frontier.partial_custody_paths.contains_key(&location.root))
    {
        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
            machine: function.machine,
            block,
            node,
            place: location.root,
        });
    }
    Ok(())
}

#[cfg(test)]
mod atomic_access_tests;

#[cfg(test)]
mod byte_observation_tests;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_current_ownership_cfg(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    _successors: &BTreeMap<BlockId, Vec<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        &BoundaryMachineDeclaration,
    >,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    call_accesses: &super::CallAccesses,
    entry: CurrentOwnership,
) -> Result<
    Vec<terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement>,
    OptimizationUnitValidationError,
> {
    let pins = super::address_joins::AddressJoinPins::new(
        function,
        functions,
        structural_types,
        call_accesses,
    );
    let mut ready = BTreeSet::from([function.entry]);
    let mut incoming = BTreeMap::<BlockId, CurrentOwnership>::new();
    incoming.insert(function.entry, entry);
    let mut returned_reference_sources = Vec::new();

    while let Some(block_id) = ready.pop_first() {
        let mut frontier = incoming
            .get(&block_id)
            .expect("reachable block has a current ownership frontier")
            .clone();

        let block = blocks[&block_id];
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index =
                u32::try_from(node_index).expect("optimization-unit node position fits u32");

            // Loan replay runs inside the same operation transaction as the
            // owned-place checks below: establishment, release, record
            // relocation, call transfer, and suspended-root access all settle
            // before the ordinary custody roster is consumed.
            let moved_reference_owner = match &node.operation {
                O::EstablishReference { source, .. } => {
                    super::references::transferred_ingress_leaf(
                        function,
                        structural_types,
                        &frontier.live_references,
                        source,
                    )
                    .map(|_| source.place)
                }
                _ => None,
            };
            super::references::apply_operation(
                function,
                functions,
                structural_types,
                block_id,
                node_index,
                &node.operation,
                &mut frontier.live_references,
            )?;
            validate_byte_observations(function, block_id, node_index, &node.operation, &frontier)?;
            validate_atomic_access(
                function,
                structural_types,
                block_id,
                node_index,
                &node.operation,
                &frontier,
            )?;

            // A store into an owned home writes the value it holds right now:
            // an owned result or an owned block arrival must still be whole
            // and live on this exact frontier, never consumed by a move or
            // an edge transfer and never mid-window.
            if let O::StructuralScalarFieldStore {
                destination:
                    destination @ (abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                        ..
                    }
                    | abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                        ..
                    }),
                ..
            } = &node.operation
                && ((destination.multiplicity() != StructuralMultiplicity::Unrestricted
                    && frontier.owned_places.get(&destination.place())
                        != Some(&destination.multiplicity()))
                    || frontier
                        .partial_custody_paths
                        .contains_key(&destination.place()))
            {
                return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                    machine: function.machine,
                    block: block_id,
                    node: node_index,
                    place: destination.place(),
                });
            }

            if let O::StructuralCase { source, .. }
            | O::StructuralCaseMembership { source, .. }
            | O::StructuralScalarFieldRead { source, .. } = &node.operation
            {
                let signature =
                    crate::unit_validation::operation_contracts::structural_source_contract(
                        function, *source,
                    )
                    .ok_or(
                        OptimizationUnitValidationError::InvalidStructuralCaseDispatch {
                            machine: function.machine,
                            source: *source,
                        },
                    )?;
                if signature.access == StructuralAccess::Owned
                    && signature.multiplicity != StructuralMultiplicity::Unrestricted
                    && (!frontier.owned_places.contains_key(source)
                        || frontier.partial_custody_paths.contains_key(source))
                {
                    return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        place: *source,
                    });
                }
            }

            if let O::EstablishTrivialAffineLocal { place, .. } = &node.operation {
                insert_owned_result(
                    function,
                    block_id,
                    node_index,
                    &mut frontier,
                    place.id,
                    StructuralMultiplicity::Affine,
                )?;
            }
            // Literal views refer to immutable image storage. Their exact
            // establishment must dominate each use, but creates no disposal
            // debt in this activation (including a re-entered block).
            if let O::ReturnStructural {
                trivial_affine_locals,
                ..
            } = &node.operation
            {
                for (_, place, _) in trivial_affine_locals {
                    insert_owned_result(
                        function,
                        block_id,
                        node_index,
                        &mut frontier,
                        place.id,
                        StructuralMultiplicity::Affine,
                    )?;
                }
            }

            // Construction joins the same whole-value transfer frontier as calls.
            // Heterogeneous field operands need a temporary structural-only roster;
            // ordinary call operands continue borrowing their retained slice.
            let record_arguments = super::structural::constructor_arguments(&node.operation)
                .cloned()
                .collect::<Vec<_>>();
            let structural_arguments = match &node.operation {
                O::EstablishCase { .. }
                | O::EstablishRecord { .. }
                | O::EstablishStructuralArray { .. } => record_arguments.as_slice(),
                // A Unit call that forwards descriptors is not replayed here;
                // the scalar-result call is replayed in both forms.
                O::CallStructural {
                    dynamic_arguments,
                    result: AbstractStructuralCallResult::Unit,
                    ..
                } if !dynamic_arguments.is_empty() => &[],
                O::CallStructural {
                    structural_arguments,
                    ..
                }
                | O::BoundaryCall {
                    structural_arguments,
                    ..
                } => structural_arguments.as_slice(),
                // A window repair consumes its value exactly like one owned
                // whole call argument: live-owned custody in, once.
                O::StoreStructuralField { value, .. } => std::slice::from_ref(value),
                O::CallDynamic {
                    dispatch:
                        abstract_operations::AbstractDynamicDispatch::Rebound(dynamic_dispatch),
                    result: abstract_operations::AbstractDynamicCallResult::Scalar(_),
                    ..
                } => std::slice::from_ref(&dynamic_dispatch.rebound.source),
                _ => &[],
            };
            let parameter_multiplicities = match &node.operation {
                O::EstablishCase { .. }
                | O::EstablishRecord { .. }
                | O::EstablishStructuralArray { .. } => record_arguments
                    .iter()
                    .map(|argument| {
                        crate::unit_validation::operation_contracts::structural_source_contract(
                            function,
                            argument.place,
                        )
                        .map(|source| source.multiplicity)
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or(
                        OptimizationUnitValidationError::StructuralCallContractMismatch {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                        },
                    )?,
                // The repair's consumed operand carries its own declared
                // signature; there is no callee parameter row to read.
                O::StoreStructuralField { value, .. } => {
                    vec![
                        crate::unit_validation::operation_contracts::structural_source_contract(
                            function,
                            value.place,
                        )
                        .map(|source| source.multiplicity)
                        .ok_or(
                            OptimizationUnitValidationError::StructuralCallContractMismatch {
                                machine: function.machine,
                                block: block_id,
                                node: node_index,
                            },
                        )?,
                    ]
                }
                O::CallStructural {
                    dynamic_arguments,
                    result: AbstractStructuralCallResult::Unit,
                    ..
                } if !dynamic_arguments.is_empty() => Vec::new(),
                O::CallStructural { callee, .. } => functions[callee]
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.multiplicity)
                    .collect::<Vec<_>>(),
                O::CallDynamic {
                    dispatch:
                        abstract_operations::AbstractDynamicDispatch::Rebound(dynamic_dispatch),
                    result: abstract_operations::AbstractDynamicCallResult::Scalar(_),
                    ..
                } => functions[&dynamic_dispatch.dispatch.realization]
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.multiplicity)
                    .collect::<Vec<_>>(),
                O::BoundaryCall { boundary, .. } => boundary_machines[boundary]
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.multiplicity)
                    .collect(),
                _ => Vec::new(),
            };
            let mut consumed_places = structural_arguments
                .iter()
                .zip(&parameter_multiplicities)
                .filter_map(|(argument, multiplicity)| {
                    (argument.path.is_empty()
                        && argument.access == StructuralAccess::Owned
                        && *multiplicity != StructuralMultiplicity::Unrestricted)
                        .then_some(argument.place)
                })
                .collect::<Vec<_>>();
            // Moving the sole ingress leaf retires the old carrier, in the
            // same transaction that preserved its loan identity above.
            consumed_places.extend(moved_reference_owner);
            // Releasing a reference consumes its carrier place; the referent
            // keeps its original home and is never disposed here. Copyable
            // carriers hold no owned-place slot — `release` already ended
            // the live reference, and there is nothing left to consume.
            if let O::ReleaseReference { source, .. } = &node.operation
                && crate::unit_validation::operation_contracts::structural_source_contract(
                    function, *source,
                )
                .is_some_and(|signature| {
                    signature.multiplicity != StructuralMultiplicity::Unrestricted
                })
            {
                consumed_places.push(*source);
            }
            pins.check_operation(
                function,
                block_id,
                node_index,
                &node.operation,
                consumed_places.iter().copied().chain(
                    structural_arguments
                        .iter()
                        .filter(|argument| {
                            !argument.path.is_empty() && argument.access == StructuralAccess::Owned
                        })
                        .map(|argument| argument.place),
                ),
            )?;
            for place in &consumed_places {
                if frontier.partial_custody_paths.contains_key(place) {
                    return Err(
                        OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: *place,
                        },
                    );
                }
            }

            let transferred = match &node.operation {
                O::CallStructural {
                    dynamic_arguments,
                    result: AbstractStructuralCallResult::Unit,
                    ..
                } if !dynamic_arguments.is_empty() => Vec::new(),
                O::CallStructural {
                    claim_transfers, ..
                } => claim_transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect::<Vec<_>>(),
                O::BoundaryCall {
                    completion_receipts,
                    ..
                } => completion_receipts
                    .iter()
                    .map(|receipt| receipt.claim)
                    .collect(),
                _ => Vec::new(),
            };
            for claim in transferred {
                if frontier.claims.remove(&claim).is_none() {
                    return Err(OptimizationUnitValidationError::CurrentClaimNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        claim,
                    });
                }
            }
            for (argument, _) in structural_arguments
                .iter()
                .zip(&parameter_multiplicities)
                .filter(|(argument, multiplicity)| {
                    (argument.access == StructuralAccess::Owned
                        && **multiplicity != StructuralMultiplicity::Unrestricted)
                        // A loan does not consume its owner, but entry parameters
                        // and produced values both require current whole/path custody.
                        || (argument.access != StructuralAccess::Owned
                            && crate::unit_validation::operation_contracts::structural_source_contract(
                                function, argument.place,
).is_some_and(|source| source.access == StructuralAccess::Owned
                                && source.multiplicity == StructuralMultiplicity::Affine))
                })
            {
                if !frontier.owned_places.contains_key(&argument.place) {
                    return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        place: argument.place,
                    });
                }
                if argument.path.is_empty()
                    && frontier.partial_custody_paths.contains_key(&argument.place)
                {
                    return Err(
                        OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: argument.place,
                        },
                    );
                }
                if !argument.path.is_empty()
                    && frontier
                        .partial_custody_paths
                        .get(&argument.place)
                        .is_some_and(|moved| {
                            moved.iter().any(|existing| {
                                existing.starts_with(&argument.path)
                                    || argument.path.starts_with(existing)
                            })
                        })
                {
                    return Err(
                        OptimizationUnitValidationError::CurrentProjectedMoveOverlap {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: argument.place,
                        },
                    );
                }
            }
            for place in consumed_places {
                if frontier.owned_places.remove(&place).is_none() {
                    return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        place,
                    });
                }
            }

            for argument in structural_arguments.iter().filter(|argument| {
                !argument.path.is_empty() && argument.access == StructuralAccess::Owned
            }) {
                if !frontier.owned_places.contains_key(&argument.place) {
                    return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
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
                    return Err(
                        OptimizationUnitValidationError::CurrentProjectedMoveOverlap {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: argument.place,
                        },
                    );
                }
                if projected_root_is_fully_consumed(
                    function,
                    structural_types,
                    &frontier,
                    argument.place,
                ) {
                    frontier.owned_places.remove(&argument.place);
                    frontier.partial_custody_paths.remove(&argument.place);
                }
            }

            // An affine descriptor carries disposal custody even when its backing
            // is borrowed. Access permission is checked independently; recording
            // its carrier here must not grant ownership of that backing.
            let result_contract =
                crate::unit_validation::operation_contracts::structural_operation_result_contract(
                    function,
                    &node.operation,
                );
            let structural_result = result_contract
                .as_ref()
                .and_then(|contract| contract.operation_result());
            if let Some(result) = structural_result {
                insert_owned_result(
                    function,
                    block_id,
                    node_index,
                    &mut frontier,
                    result.place,
                    result.multiplicity,
                )?;
                for binding in &result.claims {
                    let claim = LiveClaim {
                        input: Some(result.place),
                        path: binding.path.clone(),
                        multiplicity: Some(if binding.path.is_empty() {
                            result.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    };
                    if frontier.claims.insert(binding.claim, claim).is_some() {
                        return Err(OptimizationUnitValidationError::CurrentClaimAlreadyLive {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            claim: binding.claim,
                        });
                    }
                }
            }

            match &node.operation {
                O::Return {
                    cleanup_actions, ..
                } => {
                    reject_live_linear_claim(function, block_id, &frontier)?;
                    validate_scalar_cleanup_actions(
                        function,
                        functions,
                        structural_types,
                        block_id,
                        &frontier,
                        cleanup_actions,
                    )?;
                }
                O::ReturnUnit {
                    cleanup_actions, ..
                } => {
                    reject_live_linear_claim(function, block_id, &frontier)?;
                    validate_unit_cleanup_actions(
                        function,
                        functions,
                        structural_types,
                        block_id,
                        &frontier,
                        cleanup_actions,
                    )?;
                }
                O::ReturnStructural {
                    source,
                    returned_claims,
                    trivial_affine_discards,
                    ..
                } => {
                    // A returned carrier transfers its loans to the caller's
                    // checked result roster; the frame-local rows end here.
                    super::references::transfer_return(
                        function,
                        structural_types,
                        block_id,
                        *source,
                        &mut frontier.live_references,
                        &mut returned_reference_sources,
                    )?;
                    if frontier.partial_custody_paths.contains_key(source) {
                        return Err(
                            OptimizationUnitValidationError::CurrentStructuralReturnSourcePartiallyMoved {
                                machine: function.machine,
                                block: block_id,
                                place: *source,
                            },
                        );
                    }
                    // Structural-root validation has already matched the
                    // available source to this exact result contract. Returning
                    // a copyable value removes no disposal obligation, but its
                    // returned claims still require the exact replay below.
                    let unrestricted_result = function.result.structural().is_some_and(|result| {
                        result.multiplicity == StructuralMultiplicity::Unrestricted
                    });
                    if !unrestricted_result && frontier.owned_places.remove(source).is_none() {
                        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: *source,
                        });
                    }
                    let expected = frontier
                        .claims
                        .iter()
                        .filter_map(|(claim, live)| (live.input == Some(*source)).then_some(*claim))
                        .collect::<Vec<_>>();
                    if returned_claims != &expected {
                        return Err(
                            OptimizationUnitValidationError::CurrentStructuralReturnClaimSetMismatch {
                                machine: function.machine,
                                block: block_id,
                            },
                        );
                    }
                    for claim in returned_claims {
                        frontier.claims.remove(claim);
                    }
                    if trivial_affine_discards
                        != &expected_trivial_affine_discards(function, &frontier)
                    {
                        return Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
                            machine: function.machine,
                            block: block_id,
                        });
                    }
                    // The returned carrier's rows already transferred; every
                    // remaining leaf must end with its owner's discard.
                    for place in trivial_affine_discards {
                        super::references::discard_owned(
                            function,
                            structural_types,
                            block_id,
                            &mut frontier.live_references,
                            *place,
                        )?;
                    }
                    super::references::require_no_references(
                        function,
                        block_id,
                        &frontier.live_references,
                    )?;
                    if let Some(claim) = frontier.claims.keys().next().copied() {
                        return Err(
                            OptimizationUnitValidationError::CurrentClaimLiveAfterStructuralReturn {
                                machine: function.machine,
                                block: block_id,
                                claim,
                            },
                        );
                    }
                }
                O::Crash {
                    frontier_lower_bound,
                    ..
                } if frontier_lower_bound
                    != &frontier.claims.keys().copied().collect::<Vec<_>>() =>
                {
                    return Err(
                        OptimizationUnitValidationError::CurrentCrashClaimFrontierMismatch {
                            machine: function.machine,
                            block: block_id,
                        },
                    );
                }
                _ => {}
            }
        }

        for edge in &block
            .nodes
            .last()
            .expect("validated block is nonempty")
            .successors
        {
            pins.check_edge(
                function,
                block_id,
                u32::try_from(block.nodes.len() - 1)
                    .expect("optimization-unit node position fits u32"),
                edge,
                &blocks[&edge.target].structural_parameters,
                &frontier,
            )?;
            let mut outgoing = frontier.clone();
            if matches!(
                block
                    .nodes
                    .last()
                    .expect("validated block is nonempty")
                    .operation,
                O::Jump { .. }
            ) {
                apply_edge_partial_affine_discards(
                    function,
                    structural_types,
                    block_id,
                    &mut outgoing,
                    &edge.trivial_affine_discards,
                    &edge.residual_affine_discards,
                )?;
            }
            apply_edge_trivial_affine_discards(
                function,
                structural_types,
                block_id,
                &mut outgoing,
                &edge.trivial_affine_discards,
            )?;
            bind_owned_parameters(
                function,
                block_id,
                &mut outgoing,
                &edge.structural_bindings,
                blocks[&edge.target],
            )?;
            if let Some(existing) = incoming.get(&edge.target) {
                if existing.claims != outgoing.claims {
                    return Err(OptimizationUnitValidationError::CurrentClaimJoinMismatch {
                        machine: function.machine,
                        block: edge.target,
                    });
                }
                if existing.owned_places != outgoing.owned_places
                    || existing.partial_custody_paths != outgoing.partial_custody_paths
                    || existing.live_references != outgoing.live_references
                {
                    return Err(
                        OptimizationUnitValidationError::CurrentOwnedPlaceJoinMismatch {
                            machine: function.machine,
                            block: edge.target,
                        },
                    );
                }
            } else {
                incoming.insert(edge.target, outgoing);
                ready.insert(edge.target);
            }
        }
    }

    if let Some(block) = blocks.keys().find(|block| !incoming.contains_key(block)) {
        return Err(OptimizationUnitValidationError::UnreachableBlock {
            machine: function.machine,
            block: *block,
        });
    }

    super::references::validate_return_coverage(function, &returned_reference_sources)?;
    Ok(pins.descriptor_requirements())
}
