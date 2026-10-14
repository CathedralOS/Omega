use crate::OptimizationUnitValidationError;
use crate::current_ownership::cleanup::{
    validate_scalar_cleanup_actions, validate_unit_cleanup_actions,
};
use crate::current_ownership::mutations::{insert_owned_result, reject_live_linear_claim};
use crate::current_ownership::structural::projected_root_is_fully_consumed;
use crate::current_ownership::{
    CurrentOwnership, LiveClaim, consume_edge_sources, establish_edge_parameters,
};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    OptimizationBlock, OwnershipFrontierFact, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    PsiOptimizationFunction,
};
use semantic_vocabulary::{BlockId, MachineId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeDeclaration,
};

/// A member moved under an owned contract out of an exclusively borrowed
/// root: the leaf's disposal transfers to the callee, while the referent's
/// hole returns to its owner — absent-subtree rules replay upstream, and this
/// ledger has no custody row for the borrowed root.
fn is_borrowed_member_move(
    function: &PsiOptimizationFunction,
    argument: &terminal_psi::StructuralArgument,
) -> bool {
    !argument.path.is_empty()
        && crate::unit_validation::operation_contracts::structural_source_contract(
            function,
            argument.place,
        )
        .is_some_and(|source| source.access == StructuralAccess::MutableBorrow)
}

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
        abstract_operations::byte_len_or_field_read_pattern!(source, ..) => observe(*source),
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
    let terminal_psi::StructuralTypeShape::Record { fields, .. } =
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

/// Whether each claim names a distinct live claim
/// rooted at that owned argument's place, at or below its path. A call's
/// claim transfers and a boundary's completion receipts both hand claims over
/// this way.
fn claims_sit_at_owned_arguments<'path>(
    frontier: &CurrentOwnership,
    arguments: &[terminal_psi::StructuralArgument],
    pairs: impl IntoIterator<
        Item = (
            semantic_vocabulary::ClaimId,
            u32,
            &'path [StructuralPathSegment],
        ),
    >,
) -> bool {
    let mut seen = BTreeSet::new();
    pairs.into_iter().all(|(claim, index, path)| {
        let (Some(argument), Some(live)) =
            (arguments.get(index as usize), frontier.claims.get(&claim))
        else {
            return false;
        };
        seen.insert(claim)
            && argument.access == StructuralAccess::Owned
            && live.input == Some(argument.place)
            && live.path.starts_with(&argument.path)
            && path == &live.path[argument.path.len()..]
    })
}

/// Only independent discriminator evidence can remove a conditional ingress
/// row. A missing live identity is not evidence that its Case is inactive.
#[allow(clippy::too_many_arguments)]
fn claim_path_is_inactive(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    block: BlockId,
    node: usize,
    argument: &terminal_psi::StructuralArgument,
    path: &[StructuralPathSegment],
) -> bool {
    let Some(block_position) = function
        .blocks
        .iter()
        .position(|candidate| candidate.id == block)
    else {
        return false;
    };
    let mut projection = argument.path.clone();
    for segment in path {
        if let StructuralPathSegment::Case(case) = segment
            && crate::unit_validation::function_structure::membership_on_every_path(
                function,
                structural_types,
                predecessors,
                (block_position, node),
                argument.place,
                &projection,
                *case,
                false,
            )
        {
            return true;
        }
        projection.push(segment.clone());
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_current_ownership_cfg(
    function: &PsiOptimizationFunction,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<
        semantic_vocabulary::BoundaryMachineId,
        &BoundaryMachineDeclaration,
    >,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    call_accesses: &super::CallAccesses,
    entry: CurrentOwnership,
    psi: terminal_psi::TerminalPsiIdentity,
    facts: &mut Vec<OwnershipFrontierFact>,
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
    let mut snapshots = BTreeMap::<OwnershipFrontierSite, OwnershipFrontierSnapshot>::new();

    while let Some(block_id) = ready.pop_first() {
        let mut frontier = incoming
            .get(&block_id)
            .expect("reachable block has a current ownership frontier")
            .clone();
        snapshots.insert(
            OwnershipFrontierSite::BlockEntry(block_id),
            frontier.snapshot(),
        );

        let block = blocks[&block_id];
        let block_position = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == block_id)
            .expect("reachable block belongs to the function");
        let membership_at = |node, source, path: &[StructuralPathSegment], case, expected| {
            crate::unit_validation::function_structure::membership_on_every_path(
                function,
                structural_types,
                predecessors,
                (block_position, node),
                source,
                path,
                case,
                expected,
            )
        };
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index =
                u32::try_from(node_index).expect("optimization-unit node position fits u32");
            let psi_operation = node.operation.psi_operation();
            if let Some(operation) = psi_operation {
                snapshots
                    .entry(OwnershipFrontierSite::OperationEntry(operation))
                    .or_insert_with(|| frontier.snapshot());
            }

            // Restoration windows may repair their own temporary holes, but
            // cannot reanimate subtrees permanently consumed by an owned move
            // or descriptor release. Check the selected storage region before
            // loan replay, whose live roster no longer contains retired leaves.
            if let O::MoveStructuralField {
                source: root,
                path,
                field,
                ..
            }
            | O::StoreStructuralField {
                destination: root,
                path,
                field,
                ..
            } = &node.operation
                && let Some(consumed) = frontier.partial_custody_paths.get(&root.place())
            {
                let available = super::references::window_hole(
                    function,
                    structural_types,
                    root.place(),
                    root.structural_type(),
                    path,
                    *field,
                    match &node.operation {
                        O::StoreStructuralField { disposition, .. } => {
                            disposition.required_access()
                        }
                        _ => terminal_psi::StructuralAccess::MutableBorrow,
                    },
                )
                .is_some_and(|selected| {
                    consumed.iter().all(|absent| {
                        !terminal_semantics::structural_paths_may_overlap(absent, &selected)
                    })
                });
                if !available {
                    return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        place: root.place(),
                    });
                }
            }

            // Loan replay runs inside the same operation transaction as the
            // owned-place checks below: establishment, release, record
            // relocation, call transfer, and suspended-root access all settle
            // before the ordinary custody roster is consumed.
            let moved_reference_owner = match &node.operation {
                O::EstablishReference { source, .. }
                | O::EstablishPlacedReference { source, .. } => {
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
                dynamic_dispatch,
                structural_types,
                pins.reference_sources(),
                &call_accesses.references,
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
                let partially_unavailable =
                    frontier
                        .partial_custody_paths
                        .get(source)
                        .is_some_and(|consumed| match &node.operation {
                            O::StructuralScalarFieldRead { path, field, .. } => {
                                let observed = terminal_semantics::canonical_field_storage_region(
                                    structural_types.values().copied(),
                                    signature.structural_type,
                                    path,
                                    *field,
                                );
                                consumed.iter().any(|path| {
                                    terminal_semantics::structural_paths_may_overlap(
                                        path, &observed,
                                    )
                                })
                            }
                            O::StructuralCase { path: observed, .. }
                            | O::StructuralCaseMembership { path: observed, .. } => {
                                consumed.iter().any(|path| {
                                    terminal_semantics::structural_paths_may_overlap(path, observed)
                                })
                            }
                            _ => true,
                        });
                if signature.access == StructuralAccess::Owned
                    && signature.multiplicity != StructuralMultiplicity::Unrestricted
                    && ((!frontier.owned_places.contains_key(source)
                        && !projected_root_is_fully_consumed(
                            function,
                            structural_types,
                            &frontier,
                            *source,
                        ))
                        || partially_unavailable)
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
                abstract_operations::establish_record_or_case_pattern!(..)
                | O::EstablishStructuralArray { .. } => record_arguments.as_slice(),
                abstract_operations::call_structural_or_boundary_call_pattern!(
                    structural_arguments
                ) => structural_arguments.as_slice(),
                // A window repair consumes its value exactly like one owned
                // whole call argument: live-owned custody in, once.
                O::StoreStructuralField { value, .. } => std::slice::from_ref(value),
                _ => &[],
            };
            let parameter_multiplicities = match &node.operation {
                abstract_operations::establish_record_or_case_pattern!(..)
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
                    callee: abstract_operations::AbstractCallTarget::Machine(callee),
                    ..
                } => functions[callee]
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.multiplicity)
                    .collect::<Vec<_>>(),
                O::CallStructural {
                    callee:
                        abstract_operations::AbstractCallTarget::Dynamic {
                            interface, slot, ..
                        },
                    ..
                } => dynamic_dispatch
                    .interface(*interface)
                    .and_then(|interface| {
                        usize::try_from(*slot)
                            .ok()
                            .and_then(|slot| interface.slots.get(slot))
                    })
                    .map(|slot| {
                        slot.structural_parameters
                            .iter()
                            .map(|parameter| parameter.multiplicity)
                            .collect::<Vec<_>>()
                    })
                    .ok_or(
                        OptimizationUnitValidationError::StructuralCallContractMismatch {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                        },
                    )?,
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
            if let O::ReleaseReference { source, path, .. } = &node.operation
                && path.is_empty()
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

            if let O::CallStructural {
                callee,
                structural_arguments,
                claim_transfers,
                ..
            } = &node.operation
            {
                let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
                    machine: function.machine,
                    block: block_id,
                };
                let expected_by_argument = match callee {
                    abstract_operations::AbstractCallTarget::Machine(callee) => {
                        let target = functions.get(callee).ok_or_else(invalid)?;
                        target
                            .structural_parameters
                            .iter()
                            .map(|parameter| {
                                let mut expected = target
                                    .entry_claim_declarations
                                    .iter()
                                    .filter(|claim| claim.input == parameter.place)
                                    .map(|claim| (claim.claim, claim.path.as_slice()))
                                    .collect::<BTreeMap<_, _>>();
                                for claim in &target.content_entry_claims {
                                    if claim.input.root == parameter.place {
                                        expected.entry(claim.claim).or_insert(&[]);
                                    }
                                }
                                expected.into_values().collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                    }
                    abstract_operations::AbstractCallTarget::Dynamic {
                        interface, slot, ..
                    } => {
                        let signature = dynamic_dispatch
                            .interface(*interface)
                            .and_then(|interface| interface.slots.get(*slot as usize))
                            .ok_or_else(invalid)?;
                        signature
                            .structural_parameters
                            .iter()
                            .enumerate()
                            .map(|(argument_index, _)| {
                                signature
                                    .entry_claim_paths
                                    .iter()
                                    .filter(|claim| claim.argument_index as usize == argument_index)
                                    .map(|claim| claim.path.as_slice())
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                    }
                };
                if expected_by_argument.len() != structural_arguments.len() {
                    return Err(invalid());
                }
                for (argument_index, (argument, expected_paths)) in structural_arguments
                    .iter()
                    .zip(expected_by_argument)
                    .enumerate()
                {
                    if argument.access != StructuralAccess::Owned {
                        continue;
                    }
                    let mut expected_paths = expected_paths
                        .into_iter()
                        .filter(|expected| {
                            !claim_path_is_inactive(
                                function,
                                structural_types,
                                predecessors,
                                block_id,
                                node_index as usize,
                                argument,
                                expected,
                            )
                        })
                        .collect::<Vec<_>>();
                    let mut actual_paths = Vec::new();
                    for (identity, claim) in &frontier.claims {
                        if claim.input == Some(argument.place)
                            && claim.path.starts_with(&argument.path)
                        {
                            if claim_transfers
                                .iter()
                                .filter(|transfer| {
                                    transfer.claim == *identity
                                        && transfer.argument_index as usize == argument_index
                                        && transfer.path == claim.path[argument.path.len()..]
                                })
                                .count()
                                != 1
                            {
                                return Err(invalid());
                            }
                            actual_paths.push(&claim.path[argument.path.len()..]);
                        }
                    }
                    expected_paths.sort();
                    actual_paths.sort();
                    if expected_paths != actual_paths {
                        return Err(invalid());
                    }
                }
                if !claims_sit_at_owned_arguments(
                    &frontier,
                    structural_arguments,
                    claim_transfers.iter().map(|transfer| {
                        (
                            transfer.claim,
                            transfer.argument_index,
                            transfer.path.as_slice(),
                        )
                    }),
                ) {
                    return Err(invalid());
                }
            }
            // A boundary completion ends exactly the claims its owned
            // arguments carry, whether the caller received them as entry
            // claims or established them itself.
            if let O::BoundaryCall {
                structural_arguments,
                completion_receipts,
                ..
            } = &node.operation
            {
                let receipts_match = claims_sit_at_owned_arguments(
                    &frontier,
                    structural_arguments,
                    completion_receipts.iter().map(|receipt| {
                        (
                            receipt.claim,
                            receipt.argument_index,
                            receipt.path.as_slice(),
                        )
                    }),
                );
                // Current custody, not historical entry provenance, identifies
                // every claim the owned actual transfers to this completion.
                let complete =
                    structural_arguments
                        .iter()
                        .enumerate()
                        .all(|(argument_index, argument)| {
                            argument.access != StructuralAccess::Owned
                                || frontier.claims.iter().all(|(identity, claim)| {
                                    claim.input != Some(argument.place)
                                        || !claim.path.starts_with(&argument.path)
                                        || completion_receipts
                                            .iter()
                                            .filter(|receipt| {
                                                receipt.claim == *identity
                                                    && receipt.argument_index as usize
                                                        == argument_index
                                                    && receipt.path
                                                        == claim.path[argument.path.len()..]
                                            })
                                            .count()
                                            == 1
                                })
                        });
                if !receipts_match || !complete {
                    return Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
                        machine: function.machine,
                        block: block_id,
                    });
                }
            }
            let transferred = match &node.operation {
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
                if !frontier.owned_places.contains_key(&argument.place)
                    && !is_borrowed_member_move(function, argument)
                {
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
                    // A member carried out of an exclusively borrowed root
                    // leaves the referent's hole with its owner — no disposal
                    // row exists for the borrowed root to track here.
                    if is_borrowed_member_move(function, argument) {
                        continue;
                    }
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
                    // Disposal debt is empty, but scalar siblings retain
                    // storage and the absent-path history still fences whole uses.
                    frontier.owned_places.remove(&argument.place);
                }
            }

            if let O::ReleaseReference { source, path, .. } = &node.operation
                && !path.is_empty()
            {
                // A released descriptor becomes absent just like a transferred
                // owned subtree, but creates no borrowed-window repair debt.
                // Keep its root available for nonoverlapping scalar siblings,
                // including when no structural disposal obligations remain.
                if frontier.owned_places.get(source) != Some(&StructuralMultiplicity::Affine) {
                    return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        place: *source,
                    });
                }
                let consumed = frontier.partial_custody_paths.entry(*source).or_default();
                if consumed
                    .iter()
                    .any(|existing| existing.starts_with(path) || path.starts_with(existing))
                    || !consumed.insert(path.clone())
                {
                    return Err(
                        OptimizationUnitValidationError::CurrentProjectedMoveOverlap {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            place: *source,
                        },
                    );
                }
                if projected_root_is_fully_consumed(function, structural_types, &frontier, *source)
                {
                    frontier.owned_places.remove(source);
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

            let membership = |source, path: &[StructuralPathSegment], case, expected| {
                membership_at(node_index as usize, source, path, case, expected)
            };
            match &node.operation {
                O::Return {
                    cleanup_actions, ..
                } => {
                    frontier = validate_scalar_cleanup_actions(
                        function,
                        functions,
                        structural_types,
                        &call_accesses.references,
                        block_id,
                        &frontier,
                        cleanup_actions,
                        &membership,
                    )?;
                    reject_live_linear_claim(function, block_id, &frontier)?;
                }
                O::ReturnUnit {
                    cleanup_actions, ..
                } => {
                    validate_unit_cleanup_actions(
                        function,
                        functions,
                        structural_types,
                        &call_accesses.references,
                        block_id,
                        &frontier,
                        cleanup_actions,
                        &membership,
                    )?;
                }
                O::ReturnStructural {
                    source,
                    returned_claims,
                    cleanup_actions,
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
                        cleanup_actions,
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
                        .filter(|(_, live)| live.input == Some(*source))
                        .map(|(claim, live)| terminal_psi::StructuralResultClaimBinding {
                            claim: *claim,
                            path: live.path.clone(),
                        })
                        .collect::<Vec<_>>();
                    if returned_claims != &expected {
                        return Err(
                            OptimizationUnitValidationError::CurrentStructuralReturnClaimSetMismatch {
                                machine: function.machine,
                                block: block_id,
                            },
                        );
                    }
                    for binding in returned_claims {
                        frontier.claims.remove(&binding.claim);
                    }
                    frontier = validate_scalar_cleanup_actions(
                        function,
                        functions,
                        structural_types,
                        &call_accesses.references,
                        block_id,
                        &frontier,
                        cleanup_actions,
                        &membership,
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
            if let Some(operation) = psi_operation {
                snapshots.insert(
                    OwnershipFrontierSite::OperationExit(operation),
                    frontier.snapshot(),
                );
            }
        }

        for edge in &block
            .nodes
            .last()
            .expect("validated block is nonempty")
            .successors
        {
            snapshots.insert(
                OwnershipFrontierSite::EdgeEntry(edge.psi_edge),
                frontier.snapshot(),
            );
            pins.check_edge(
                function,
                block_id,
                u32::try_from(block.nodes.len() - 1)
                    .expect("optimization-unit node position fits u32"),
                edge,
                &blocks[&edge.target].structural_parameters,
                &frontier,
            )?;
            // Owned sources leave first, so a projected move opens partial
            // custody that this edge's residual cleanup can then retire; the
            // target's roots are installed only after that cleanup.
            let mut outgoing = frontier.clone();
            if let O::StructuralCase {
                source,
                path,
                cases,
            } = &block
                .nodes
                .last()
                .ok_or(OptimizationUnitValidationError::CurrentCleanupMismatch {
                    machine: function.machine,
                    block: block_id,
                })?
                .operation
            {
                let selected = cases
                    .iter()
                    .find(|case| case.psi_edge == edge.psi_edge)
                    .ok_or(OptimizationUnitValidationError::CurrentCleanupMismatch {
                        machine: function.machine,
                        block: block_id,
                    })?;
                // This edge authenticates one discriminator. Conditional
                // catalogue rows for other payloads are not live obligations
                // on the selected edge; common members remain unconditional.
                outgoing.claims.retain(|_, claim| {
                    if claim.input != Some(*source) {
                        return true;
                    }
                    match claim
                        .path
                        .strip_prefix(path.as_slice())
                        .and_then(|suffix| suffix.first())
                    {
                        Some(StructuralPathSegment::Case(case)) => *case == selected.case,
                        _ => true,
                    }
                });
            }
            let membership = |source, path: &[StructuralPathSegment], case, expected| {
                let terminator = &block.nodes[block.nodes.len() - 1].operation;
                if let O::StructuralCase {
                    source: observed,
                    path: observed_path,
                    cases,
                } = terminator
                    && *observed == source
                    && observed_path == path
                    && let Some(selected) = cases
                        .iter()
                        .find(|selected| selected.psi_edge == edge.psi_edge)
                {
                    return (selected.case == case) == expected;
                }
                membership_at(block.nodes.len() - 1, source, path, case, expected)
            };
            let transfer = consume_edge_sources(
                function,
                structural_types,
                block_id,
                &mut outgoing,
                &edge.structural_bindings,
                blocks[&edge.target],
                |source, path, case| membership(source, path, case, false),
            )?;
            super::continuation::apply_edge_cleanup_actions(
                function,
                functions,
                structural_types,
                block_id,
                &mut outgoing,
                &edge.cleanup_actions,
                &membership,
            )?;
            establish_edge_parameters(
                function,
                block_id,
                &mut outgoing,
                blocks[&edge.target],
                transfer,
            )?;
            snapshots.insert(
                OwnershipFrontierSite::EdgeExit(edge.psi_edge),
                outgoing.snapshot(),
            );
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
    facts.extend(
        snapshots.into_iter().map(|(site, snapshot)| {
            OwnershipFrontierFact::new(psi, function.machine, site, snapshot)
        }),
    );
    Ok(pins.descriptor_requirements())
}
