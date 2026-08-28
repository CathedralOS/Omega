use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::{AbstractFunctionResult, AbstractOperation as O};
use omega_optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
use psi_core::{BlockId, ClaimId, MachineId, PlaceId, StructuralTypeId};
use psi_terminal::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralAffineDiscard, StructuralFieldType,
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

use crate::OptimizationUnitValidationError;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveClaim {
    input: Option<PlaceId>,
    path: Vec<StructuralPathSegment>,
    multiplicity: Option<StructuralMultiplicity>,
}

/// Executable ownership reconstructed from current operations and signatures.
/// Immutable source snapshots and cached `OwnershipEvent` rows are not read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CurrentOwnership {
    claims: BTreeMap<ClaimId, LiveClaim>,
    owned_places: BTreeMap<PlaceId, StructuralMultiplicity>,
    partial_custody_paths: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>>,
}

pub(super) fn validate_current_ownership_frontier(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<psi_core::BoundaryMachineId, &BoundaryMachineDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut claims = BTreeMap::<ClaimId, LiveClaim>::new();
    for claim in &function.entry_claim_declarations {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
            .expect("structural signature validation precedes current ownership replay");
        claims.insert(
            claim.claim,
            LiveClaim {
                input: Some(claim.input),
                path: claim.path.clone(),
                multiplicity: Some(if claim.path.is_empty() {
                    parameter.multiplicity
                } else {
                    StructuralMultiplicity::Linear
                }),
            },
        );
    }
    for claim in &function.content_entry_claims {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input.root);
        claims.entry(claim.claim).or_insert(LiveClaim {
            input: parameter.map(|_| claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
        });
    }
    let entry = CurrentOwnership {
        claims,
        owned_places: function
            .structural_parameters
            .iter()
            .filter_map(|parameter| {
                (parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    .then_some((parameter.place, parameter.multiplicity))
            })
            .collect(),
        partial_custody_paths: BTreeMap::new(),
    };

    let mut predecessor_edges = blocks
        .keys()
        .map(|block| (*block, 0usize))
        .collect::<BTreeMap<_, _>>();
    for targets in successors.values() {
        for target in targets {
            *predecessor_edges
                .get_mut(target)
                .expect("CFG validation precedes current ownership replay") += 1;
        }
    }
    let mut ready = predecessor_edges
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut incoming = BTreeMap::<BlockId, Vec<CurrentOwnership>>::new();
    incoming.insert(function.entry, vec![entry]);

    while let Some(block_id) = ready.pop_first() {
        let frontiers = incoming
            .remove(&block_id)
            .expect("total reachable acyclic CFG has a current ownership frontier");
        let mut frontier = frontiers
            .first()
            .expect("reachable block has an incoming ownership frontier")
            .clone();
        if frontiers
            .iter()
            .any(|candidate| candidate.claims != frontier.claims)
        {
            return Err(OptimizationUnitValidationError::CurrentClaimJoinMismatch {
                machine: function.machine,
                block: block_id,
            });
        }
        if frontiers.iter().any(|candidate| {
            candidate.owned_places != frontier.owned_places
                || candidate.partial_custody_paths != frontier.partial_custody_paths
        }) {
            return Err(
                OptimizationUnitValidationError::CurrentOwnedPlaceJoinMismatch {
                    machine: function.machine,
                    block: block_id,
                },
            );
        }

        let block = blocks[&block_id];
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index =
                u32::try_from(node_index).expect("optimization-unit node position fits u32");

            if let O::BooleanStructuralField { source, .. } = node.operation
                && (!frontier.owned_places.contains_key(&source)
                    || frontier.partial_custody_paths.contains_key(&source))
            {
                return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
                    machine: function.machine,
                    block: block_id,
                    node: node_index,
                    place: source,
                });
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

            let structural_arguments = match &node.operation {
                O::CallUnit {
                    structural_arguments,
                    ..
                }
                | O::CallStructuralScalar {
                    structural_arguments,
                    ..
                }
                | O::CallStructural {
                    structural_arguments,
                    ..
                }
                | O::BoundaryCall {
                    structural_arguments,
                    ..
                } => structural_arguments.as_slice(),
                _ => &[],
            };
            let parameter_multiplicities = match &node.operation {
                O::CallUnit { callee, .. }
                | O::CallStructuralScalar { callee, .. }
                | O::CallStructural { callee, .. } => functions[callee]
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
            let consumed_places = structural_arguments
                .iter()
                .zip(&parameter_multiplicities)
                .filter_map(|(argument, multiplicity)| {
                    (argument.path.is_empty()
                        && *multiplicity != StructuralMultiplicity::Unrestricted)
                        .then_some(argument.place)
                })
                .collect::<Vec<_>>();
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
                O::CallUnit {
                    claim_transfers, ..
                }
                | O::CallStructuralScalar {
                    claim_transfers, ..
                }
                | O::CallStructural {
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
                if projected_fixed_array_root_is_fully_consumed(
                    function,
                    structural_types,
                    &frontier,
                    argument.place,
                ) {
                    frontier.owned_places.remove(&argument.place);
                    frontier.partial_custody_paths.remove(&argument.place);
                }
            }

            let structural_result = match &node.operation {
                O::EstablishPayloadlessCase { result, .. } | O::CallStructural { result, .. } => {
                    Some(result)
                }
                _ => None,
            };
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
                    if frontier.partial_custody_paths.contains_key(source) {
                        return Err(
                            OptimizationUnitValidationError::CurrentStructuralReturnSourcePartiallyMoved {
                                machine: function.machine,
                                block: block_id,
                                place: *source,
                            },
                        );
                    }
                    if frontier.owned_places.remove(source).is_none() {
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
                } => {
                    if frontier_lower_bound != &frontier.claims.keys().copied().collect::<Vec<_>>()
                    {
                        return Err(
                            OptimizationUnitValidationError::CurrentCrashClaimFrontierMismatch {
                                machine: function.machine,
                                block: block_id,
                            },
                        );
                    }
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
            let mut outgoing = frontier.clone();
            apply_edge_trivial_affine_discards(
                function,
                block_id,
                &mut outgoing,
                &edge.trivial_affine_discards,
            )?;
            incoming.entry(edge.target).or_default().push(outgoing);
            let count = predecessor_edges
                .get_mut(&edge.target)
                .expect("validated edge target is indexed");
            *count -= 1;
            if *count == 0 {
                ready.insert(edge.target);
            }
        }
    }

    Ok(())
}

fn insert_owned_result(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    frontier: &mut CurrentOwnership,
    place: PlaceId,
    multiplicity: StructuralMultiplicity,
) -> Result<(), OptimizationUnitValidationError> {
    if frontier.owned_places.insert(place, multiplicity).is_some() {
        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
            machine: function.machine,
            block,
            node,
            place,
        });
    }
    Ok(())
}

fn reject_live_linear_claim(
    function: &PsiOptimizationFunction,
    block: BlockId,
    frontier: &CurrentOwnership,
) -> Result<(), OptimizationUnitValidationError> {
    if let Some(claim) = frontier.claims.iter().find_map(|(claim, live)| {
        (live.multiplicity == Some(StructuralMultiplicity::Linear)).then_some(*claim)
    }) {
        return Err(
            OptimizationUnitValidationError::CurrentLinearClaimAtReturn {
                machine: function.machine,
                block,
                claim,
            },
        );
    }
    Ok(())
}

fn expected_trivial_affine_discards(
    function: &PsiOptimizationFunction,
    frontier: &CurrentOwnership,
) -> Vec<PlaceId> {
    let mut output = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if frontier.owned_places.contains_key(&place.id) => {
                Some((declaration_ordinal, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    output.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    let mut output = output
        .into_iter()
        .map(|(_, place)| place)
        .collect::<Vec<_>>();
    output.extend(
        function
            .structural_parameters
            .iter()
            .rev()
            .filter_map(|parameter| {
                (parameter.multiplicity == StructuralMultiplicity::Affine
                    && frontier.owned_places.contains_key(&parameter.place)
                    && !frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(parameter.place))
                    && !function
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == parameter.place))
                .then_some(parameter.place)
            }),
    );
    output
}

fn apply_edge_trivial_affine_discards(
    function: &PsiOptimizationFunction,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    discards: &[PlaceId],
) -> Result<(), OptimizationUnitValidationError> {
    let eligible = expected_trivial_affine_discards(function, frontier);
    let mut next = 0;
    for eligible_place in eligible {
        if discards.get(next) == Some(&eligible_place) {
            next += 1;
        }
    }
    if next != discards.len() {
        return Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
            machine: function.machine,
            block,
        });
    }
    for place in discards {
        frontier.owned_places.remove(place);
    }
    Ok(())
}

fn validate_unit_cleanup_actions(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &CurrentOwnership,
    actions: &[TerminalAffineCleanupAction],
) -> Result<(), OptimizationUnitValidationError> {
    let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    let has_residual = actions
        .iter()
        .any(|action| matches!(action, TerminalAffineCleanupAction::DiscardResidual(_)));
    let has_nominal = actions
        .iter()
        .any(|action| matches!(action, TerminalAffineCleanupAction::InvokeNominal(_)));
    if has_residual && has_nominal {
        return Err(mismatch());
    }

    if has_residual {
        let first_residual = actions
            .iter()
            .position(|action| matches!(action, TerminalAffineCleanupAction::DiscardResidual(_)))
            .expect("residual action was observed");
        let roots = actions[..first_residual]
            .iter()
            .map(|action| match action {
                TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                TerminalAffineCleanupAction::DiscardResidual(_)
                | TerminalAffineCleanupAction::InvokeNominal(_) => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(mismatch)?;
        let residuals = actions[first_residual..]
            .iter()
            .map(|action| match action {
                TerminalAffineCleanupAction::DiscardResidual(discard) => Some(discard),
                TerminalAffineCleanupAction::DiscardRoot(_)
                | TerminalAffineCleanupAction::InvokeNominal(_) => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(mismatch)?;
        let root = residuals.first().ok_or_else(mismatch)?.place;
        if residuals.iter().any(|discard| discard.place != root) {
            return Err(mismatch());
        }
        let mut remaining = frontier.clone();
        let moved = remaining
            .partial_custody_paths
            .remove(&root)
            .ok_or_else(mismatch)?;
        let root_type = place_structural_type(function, root).ok_or_else(mismatch)?;
        let expected =
            partial_affine_residuals(structural_types, root_type, &moved).ok_or_else(mismatch)?;
        if moved.is_empty()
            || residuals.len() != expected.len()
            || residuals
                .iter()
                .zip(expected)
                .any(|(actual, (path, structural_type))| {
                    actual.path != path || actual.structural_type != structural_type
                })
            || remaining.owned_places.remove(&root) != Some(StructuralMultiplicity::Affine)
            || roots != expected_trivial_affine_discards(function, &remaining)
            || !remaining.partial_custody_paths.is_empty()
        {
            return Err(mismatch());
        }
        return Ok(());
    }

    if has_nominal {
        let mut remaining = frontier.clone();
        for action in actions {
            let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
                return Err(mismatch());
            };
            if cleanup.cleanup_receiver.is_some()
                || !cleanup.requirement_obligations.is_empty()
                || remaining
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(cleanup.place))
                || remaining.owned_places.remove(&cleanup.place)
                    != Some(StructuralMultiplicity::Affine)
                || place_structural_type(function, cleanup.place) != Some(cleanup.structural_type)
                || !valid_nominal_cleanup(function, functions, structural_types, cleanup)
            {
                return Err(mismatch());
            }
        }
        if !remaining.partial_custody_paths.is_empty()
            || !remaining.claims.is_empty()
            || !remaining.owned_places.is_empty()
        {
            return Err(mismatch());
        }
        return Ok(());
    }

    let roots = actions
        .iter()
        .map(|action| match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
            TerminalAffineCleanupAction::DiscardResidual(_)
            | TerminalAffineCleanupAction::InvokeNominal(_) => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(mismatch)?;
    if !frontier.partial_custody_paths.is_empty()
        || roots != expected_trivial_affine_discards(function, frontier)
    {
        return Err(mismatch());
    }
    Ok(())
}

fn validate_scalar_cleanup_actions(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &CurrentOwnership,
    actions: &[TerminalAffineCleanupAction],
) -> Result<(), OptimizationUnitValidationError> {
    let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    let mut remaining = frontier.clone();
    let mut actions = actions.iter();

    let mut locals = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if remaining.owned_places.contains_key(&place.id) => {
                Some((declaration_ordinal, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    locals.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    for (_, place) in locals {
        if actions.next() != Some(&TerminalAffineCleanupAction::DiscardRoot(place)) {
            return Err(mismatch());
        }
        remaining.owned_places.remove(&place);
    }

    for parameter in function.structural_parameters.iter().rev() {
        if !remaining.owned_places.contains_key(&parameter.place) {
            continue;
        }
        if parameter.multiplicity != StructuralMultiplicity::Affine
            || remaining
                .claims
                .values()
                .any(|claim| claim.input == Some(parameter.place))
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == parameter.place)
        {
            return Err(mismatch());
        }
        if let Some(moved) = remaining.partial_custody_paths.remove(&parameter.place) {
            let residuals =
                partial_affine_residuals(structural_types, parameter.structural_type, &moved)
                    .ok_or_else(mismatch)?;
            if moved.is_empty() || residuals.is_empty() {
                return Err(mismatch());
            }
            for (path, structural_type) in residuals {
                let expected =
                    TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                        place: parameter.place,
                        path,
                        structural_type,
                    });
                if actions.next() != Some(&expected) {
                    return Err(mismatch());
                }
            }
        } else {
            let Some(action) = actions.next() else {
                return Err(mismatch());
            };
            match action {
                TerminalAffineCleanupAction::DiscardRoot(place) if *place == parameter.place => {}
                TerminalAffineCleanupAction::InvokeNominal(cleanup)
                    if cleanup.place == parameter.place
                        && cleanup.structural_type == parameter.structural_type
                        && cleanup.cleanup_receiver.is_none()
                        && cleanup.requirement_obligations.is_empty()
                        && valid_nominal_cleanup(
                            function,
                            functions,
                            structural_types,
                            cleanup,
                        ) => {}
                TerminalAffineCleanupAction::DiscardRoot(_)
                | TerminalAffineCleanupAction::DiscardResidual(_)
                | TerminalAffineCleanupAction::InvokeNominal(_) => return Err(mismatch()),
            }
        }
        remaining.owned_places.remove(&parameter.place);
    }

    if actions.next().is_some()
        || !remaining.owned_places.is_empty()
        || !remaining.partial_custody_paths.is_empty()
    {
        return Err(mismatch());
    }
    Ok(())
}

fn valid_nominal_cleanup(
    caller: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cleanup: &psi_terminal::NominalAffineCleanup,
) -> bool {
    let Some(source) = structural_types.get(&cleanup.structural_type) else {
        return false;
    };
    let Some(target) = functions.get(&cleanup.cleanup_machine).copied() else {
        return false;
    };
    cleanup.cleanup_machine != caller.machine
        && bounded_nominal_cleanup_receiver_shape(&source.shape)
        && target.attachment == Some(cleanup.structural_type)
        && matches!(target.result, AbstractFunctionResult::Unit)
        && target.parameters.is_empty()
        && target.structural_parameters.is_empty()
        && target.structural_places.is_empty()
        && target.entry_claim_declarations.is_empty()
        && target.content_entry_claims.is_empty()
        && target.published_service_ceiling.is_empty()
        && target.verified_contract.as_ref().is_none_or(|contract| {
            contract.ensures.is_empty()
                && contract.outcome_specific_ensures.is_empty()
                && contract.crash_routes.is_empty()
        })
}

fn bounded_nominal_cleanup_receiver_shape(shape: &StructuralTypeShape) -> bool {
    let StructuralTypeShape::Record { fields } = shape else {
        return false;
    };
    fields.iter().all(|field| {
        !field.relevance.is_erased()
            && match field.field_type {
                StructuralFieldType::Scalar(psi_core::ScalarType::Boolean) => true,
                StructuralFieldType::Scalar(psi_core::ScalarType::Integer(integer)) => {
                    matches!(integer.bits(), 8 | 16 | 32 | 64)
                        && (!integer.is_address() || integer.bits() == 64)
                }
                StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::ByteSequence(_)
                | StructuralFieldType::Structural(_)
                | StructuralFieldType::Erased { .. } => false,
            }
    })
}

fn place_structural_type(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    function
        .structural_parameters
        .iter()
        .find_map(|parameter| (parameter.place == place).then_some(parameter.structural_type))
        .or_else(|| {
            function
                .result
                .structural()
                .filter(|result| result.place == place)
                .map(|result| result.structural_type)
        })
        .or_else(|| {
            function
                .structural_places
                .iter()
                .find_map(|candidate| (candidate.id == place).then_some(candidate.kind))
                .and_then(|kind| match kind {
                    psi_core::StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    }
                    | psi_core::StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    }
                    | psi_core::StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    } => Some(structural_type),
                    psi_core::StructuralPlaceKind::ProviderAttachment { attachment, .. } => {
                        Some(attachment)
                    }
                    psi_core::StructuralPlaceKind::Parameter { .. }
                    | psi_core::StructuralPlaceKind::Result => None,
                })
        })
}

fn projected_fixed_array_root_is_fully_consumed(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    frontier: &CurrentOwnership,
    place: PlaceId,
) -> bool {
    let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    else {
        return false;
    };
    if frontier
        .claims
        .values()
        .any(|claim| claim.input == Some(place))
    {
        return false;
    }
    let Some(StructuralTypeShape::FixedArray { element, length }) = structural_types
        .get(&parameter.structural_type)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let Some(length) = usize::try_from(*length).ok() else {
        return false;
    };
    if parameter.multiplicity != StructuralMultiplicity::Linear
        && (parameter.multiplicity != StructuralMultiplicity::Affine
            || parameter.is_self
            || parameter.access != StructuralAccess::Owned
            || !parameter.qualifications.is_empty()
            || length != 2
            || !matches!(
                structural_types
                    .get(element)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Record { .. })
            ))
    {
        return false;
    }
    let Some(moved) = frontier.partial_custody_paths.get(&place) else {
        return false;
    };
    moved.len() == length
        && (0..length).all(|index| {
            moved.contains(&vec![StructuralPathSegment::FixedIndex(
                u64::try_from(index).expect("a usize index fits u64"),
            )])
        })
}

fn partial_affine_residuals(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved_paths.is_empty()
        || moved_paths
            .iter()
            .any(|path| !is_bounded_partial_affine_path(structural_types, root_type, path))
    {
        return None;
    }
    if moved_paths
        .iter()
        .all(|path| matches!(path.as_slice(), [StructuralPathSegment::FixedIndex(_)]))
    {
        let StructuralTypeShape::FixedArray { element, length } =
            structural_types.get(&root_type)?.shape
        else {
            return None;
        };
        if !matches!(
            structural_types
                .get(&element)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::Record { .. })
        ) {
            return None;
        }
        match length {
            2 if moved_paths.len() == 1 => {
                let [StructuralPathSegment::FixedIndex(index @ (0 | 1))] =
                    moved_paths.first()?.as_slice()
                else {
                    return None;
                };
                return Some(vec![(
                    vec![StructuralPathSegment::FixedIndex(1 - index)],
                    element,
                )]);
            }
            3 if moved_paths.len() == 2 => {
                let moved = moved_paths
                    .iter()
                    .filter_map(|path| match path.as_slice() {
                        [StructuralPathSegment::FixedIndex(index @ (0 | 1 | 2))] => Some(*index),
                        _ => None,
                    })
                    .collect::<BTreeSet<_>>();
                if moved.len() != 2 {
                    return None;
                }
                let residual = (0_u64..3).find(|index| !moved.contains(index))?;
                return Some(vec![(
                    vec![StructuralPathSegment::FixedIndex(residual)],
                    element,
                )]);
            }
            _ => return None,
        }
    }
    if moved_paths.iter().enumerate().any(|(index, path)| {
        moved_paths
            .iter()
            .enumerate()
            .any(|(other_index, other)| index != other_index && path.starts_with(other))
    }) {
        return None;
    }
    let moved_paths = moved_paths.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut residuals = Vec::new();
    collect_partial_affine_residuals(
        structural_types,
        root_type,
        &moved_paths,
        &mut Vec::new(),
        &mut residuals,
    )?;
    Some(residuals)
}

fn is_bounded_partial_affine_path(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> bool {
    matches!(path, [StructuralPathSegment::Field(_), ..])
        || (matches!(path, [StructuralPathSegment::FixedIndex(_)])
            && structural_types.get(&root_type).is_some_and(|declaration| {
                matches!(
                    (&declaration.shape, path),
                    (
                        StructuralTypeShape::FixedArray { length: 2, .. },
                        [StructuralPathSegment::FixedIndex(0 | 1)]
                    ) | (
                        StructuralTypeShape::FixedArray { length: 3, .. },
                        [StructuralPathSegment::FixedIndex(0 | 1 | 2)]
                    )
                )
            }))
}

fn collect_partial_affine_residuals(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    moved_paths: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    let StructuralTypeShape::Record { fields } = &structural_types.get(&structural_type)?.shape
    else {
        return None;
    };
    if fields.is_empty()
        || fields.iter().any(|field| {
            field.relevance.is_erased()
                || !matches!(
                    field.field_type,
                    StructuralFieldType::Structural(_)
                        | StructuralFieldType::Scalar(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                )
        })
    {
        return None;
    }
    let mut matched = 0_usize;
    for field in fields.iter().rev() {
        prefix.push(StructuralPathSegment::Field(field.identity.clone()));
        let descendants = moved_paths
            .iter()
            .filter_map(|path| match path {
                [StructuralPathSegment::Field(identity), remaining @ ..]
                    if *identity == field.identity =>
                {
                    Some(remaining)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        matched += descendants.len();
        let StructuralFieldType::Structural(field_type) = field.field_type else {
            if !descendants.is_empty() {
                return None;
            }
            prefix.pop();
            continue;
        };
        if descendants.is_empty() {
            residuals.push((prefix.clone(), field_type));
        } else if descendants.iter().all(|path| !path.is_empty()) {
            collect_partial_affine_residuals(
                structural_types,
                field_type,
                &descendants,
                prefix,
                residuals,
            )?;
        } else if descendants.len() != 1 {
            return None;
        }
        prefix.pop();
    }
    (matched == moved_paths.len()).then_some(())
}
