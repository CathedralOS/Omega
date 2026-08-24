use super::affine_cleanup::{
    bounded_nominal_cleanup_receiver_shape, valid_nominal_cleanup_requirements,
};
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveClaim {
    input: Option<PlaceId>,
    path: Vec<StructuralPathSegment>,
    multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StructuralOwnershipFrontier {
    // Claims carry proof-visible custody identity. Owned places independently
    // enforce by-value affine/linear use even when no linear claim row exists.
    claims: BTreeMap<ClaimId, LiveClaim>,
    owned_places: BTreeMap<PlaceId, StructuralMultiplicity>,
    /// Exact field paths already transferred from an otherwise-live affine
    /// root. The root remains present until its complementary residual action
    /// proves complete exhaustion at `ReturnUnitPartialAffine`.
    moved_field_paths: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>>,
}

pub(super) fn validate_structural_frontier(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
) -> Result<(), ModuleError> {
    let mut claims = BTreeMap::<ClaimId, LiveClaim>::new();
    for claim in &machine.entry_claims {
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
            .expect("entry claims were validated against structural parameters");
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
    for claim in &machine.content_entry_claims {
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input.root);
        claims.entry(claim.claim).or_insert(LiveClaim {
            input: parameter.map(|_| claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
        });
    }
    let entry = StructuralOwnershipFrontier {
        claims,
        owned_places: machine
            .structural_parameters
            .iter()
            .filter_map(|parameter| {
                (parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    .then_some((parameter.place, parameter.multiplicity))
            })
            .collect(),
        moved_field_paths: BTreeMap::new(),
    };

    let mut successors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut predecessors = blocks
        .keys()
        .map(|block| (*block, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for block in blocks.values() {
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        for target in &targets {
            *predecessors
                .get_mut(target)
                .expect("control validation established every target") += 1;
        }
        successors.insert(block.id, targets);
    }
    let mut ready = predecessors
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(block) = ready.pop_first() {
        order.push(block);
        for target in &successors[&block] {
            let count = predecessors
                .get_mut(target)
                .expect("control validation established every target");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }

    let mut incoming = BTreeMap::<BlockId, Vec<StructuralOwnershipFrontier>>::new();
    incoming.insert(machine.entry, vec![entry]);
    for block_id in order {
        let frontiers = incoming
            .remove(&block_id)
            .expect("control validation established reachability");
        let frontier = frontiers
            .first()
            .expect("a reachable block has an incoming frontier")
            .clone();
        if frontiers
            .iter()
            .any(|candidate| candidate.claims != frontier.claims)
        {
            return Err(ModuleError::ClaimFrontierJoinMismatch(block_id));
        }
        if frontiers
            .iter()
            .any(|candidate| candidate.owned_places != frontier.owned_places)
            || frontiers
                .iter()
                .any(|candidate| candidate.moved_field_paths != frontier.moved_field_paths)
        {
            return Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block_id));
        }
        let block = blocks
            .get(&block_id)
            .expect("topological order contains known blocks");
        let mut frontier = frontier;
        for operation in &block.operations {
            if let OperationKind::BooleanStructuralField { source, .. } = operation.kind
                && (!frontier.owned_places.contains_key(&source)
                    || frontier.moved_field_paths.contains_key(&source))
            {
                return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: operation.id,
                    place: source,
                });
            }
            if let OperationKind::EstablishTrivialAffineLocal { destination } = operation.kind {
                if frontier
                    .owned_places
                    .insert(destination, StructuralMultiplicity::Affine)
                    .is_some()
                {
                    return Err(ModuleError::TrivialAffineLocalAlreadyLive {
                        operation: operation.id,
                        place: destination,
                    });
                }
            }
            let claims = match &operation.kind {
                OperationKind::CallUnit {
                    claim_transfers, ..
                }
                | OperationKind::CallStructuralScalar {
                    claim_transfers, ..
                }
                | OperationKind::CallStructural {
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
            };
            for claim in claims {
                if frontier.claims.remove(&claim).is_none() {
                    return Err(ModuleError::ClaimNotLiveAtOperation {
                        operation: operation.id,
                        claim,
                    });
                }
            }
            let consumed_places = match &operation.kind {
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
                } => structural_arguments
                    .iter()
                    .zip(&machines[callee].structural_parameters)
                    .filter_map(|(argument, parameter)| {
                        (argument.path.is_empty()
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
                                && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                                .then_some(argument.place)
                        })
                        .collect()
                }
                _ => Vec::new(),
            };
            for place in consumed_places {
                if frontier.owned_places.remove(&place).is_none() {
                    return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                        operation: operation.id,
                        place,
                    });
                }
            }
            let projected_arguments = match &operation.kind {
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
                | OperationKind::BoundaryCall {
                    structural_arguments,
                    ..
                } => structural_arguments.as_slice(),
                _ => &[],
            };
            for argument in projected_arguments
                .iter()
                .filter(|argument| !argument.path.is_empty())
            {
                if is_nonempty_field_path(&argument.path)
                    && matches!(block.terminator, Terminator::ReturnUnitPartialAffine { .. })
                {
                    let moved = frontier
                        .moved_field_paths
                        .entry(argument.place)
                        .or_default();
                    if !frontier.owned_places.contains_key(&argument.place)
                        || moved.iter().any(|existing| {
                            existing.starts_with(&argument.path)
                                || argument.path.starts_with(existing)
                        })
                        || !moved.insert(argument.path.clone())
                    {
                        return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                            operation: operation.id,
                            place: argument.place,
                        });
                    }
                    continue;
                }
                if !frontier
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(argument.place))
                    && frontier.owned_places.remove(&argument.place).is_none()
                {
                    return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                        operation: operation.id,
                        place: argument.place,
                    });
                }
            }
            if let OperationResult::Structural(result) = &operation.result {
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
        }
        match &block.terminator {
            Terminator::Jump {
                edge,
                target,
                trivial_affine_discards,
                ..
            } => {
                apply_edge_trivial_affine_discards(
                    machine,
                    &mut frontier,
                    *edge,
                    trivial_affine_discards,
                )?;
                incoming.entry(*target).or_default().push(frontier);
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                let mut true_frontier = frontier.clone();
                apply_edge_trivial_affine_discards(
                    machine,
                    &mut true_frontier,
                    when_true.edge,
                    &when_true.trivial_affine_discards,
                )?;
                incoming
                    .entry(when_true.target)
                    .or_default()
                    .push(true_frontier);
                apply_edge_trivial_affine_discards(
                    machine,
                    &mut frontier,
                    when_false.edge,
                    &when_false.trivial_affine_discards,
                )?;
                incoming
                    .entry(when_false.target)
                    .or_default()
                    .push(frontier);
            }
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => {
                let expected_affine_discards = expected_trivial_affine_discards(machine, &frontier);
                if *trivial_affine_discards != expected_affine_discards {
                    return Err(ModuleError::UnitReturnAffineDiscardsMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                if let Some((claim, _)) = frontier
                    .claims
                    .iter()
                    .find(|(_, claim)| claim.multiplicity == Some(StructuralMultiplicity::Linear))
                {
                    return Err(ModuleError::LiveLinearClaimAtUnitReturn {
                        machine: machine.id,
                        block: block.id,
                        claim: *claim,
                    });
                }
            }
            Terminator::ReturnUnitPartialAffine {
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } => {
                let Some(first_residual) = residual_affine_discards.first() else {
                    return Err(ModuleError::InvalidPartialAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                };
                let root_place = first_residual.place;
                if residual_affine_discards
                    .iter()
                    .any(|residual| residual.place != root_place)
                {
                    return Err(ModuleError::InvalidPartialAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                let Some(moved) = frontier.moved_field_paths.remove(&root_place) else {
                    return Err(ModuleError::InvalidPartialAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                };
                let expected_residuals = machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == root_place)
                    .and_then(|parameter| {
                        partial_affine_residuals(module, parameter.structural_type, &moved)
                    });
                if moved.is_empty()
                    || expected_residuals.as_ref().is_none_or(|expected| {
                        residual_affine_discards.len() != expected.len()
                            || residual_affine_discards.iter().zip(expected).any(
                                |(residual, (path, structural_type))| {
                                    residual.path != *path
                                        || residual.structural_type != *structural_type
                                },
                            )
                    })
                    || frontier.owned_places.remove(&root_place)
                        != Some(StructuralMultiplicity::Affine)
                {
                    return Err(ModuleError::InvalidPartialAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                let expected_affine_discards = expected_trivial_affine_discards(machine, &frontier);
                if *trivial_affine_discards != expected_affine_discards {
                    return Err(ModuleError::UnitReturnAffineDiscardsMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                if !frontier.moved_field_paths.is_empty() {
                    return Err(ModuleError::InvalidPartialAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                if let Some((claim, _)) = frontier
                    .claims
                    .iter()
                    .find(|(_, claim)| claim.multiplicity == Some(StructuralMultiplicity::Linear))
                {
                    return Err(ModuleError::LiveLinearClaimAtUnitReturn {
                        machine: machine.id,
                        block: block.id,
                        claim: *claim,
                    });
                }
            }
            Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
                for cleanup in cleanups {
                    if frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(cleanup.place))
                        || frontier.owned_places.remove(&cleanup.place)
                            != Some(StructuralMultiplicity::Affine)
                    {
                        return Err(ModuleError::InvalidNominalAffineCleanup {
                            machine: machine.id,
                            block: block.id,
                        });
                    }
                }
                if !frontier.moved_field_paths.is_empty()
                    || !frontier.claims.is_empty()
                    || !frontier.owned_places.is_empty()
                {
                    return Err(ModuleError::InvalidNominalAffineCleanup {
                        machine: machine.id,
                        block: block.id,
                    });
                }
            }
            Terminator::Return {
                cleanup_actions, ..
            } => {
                if let Some((claim, _)) = frontier
                    .claims
                    .iter()
                    .find(|(_, claim)| claim.multiplicity == Some(StructuralMultiplicity::Linear))
                {
                    return Err(ModuleError::LiveLinearClaimAtScalarReturn {
                        machine: machine.id,
                        block: block.id,
                        claim: *claim,
                    });
                }
                validate_scalar_cleanup_actions(
                    module,
                    machine,
                    machines,
                    block.id,
                    &frontier,
                    cleanup_actions,
                )?;
            }
            Terminator::ReturnStructural {
                source,
                returned_claims,
                trivial_affine_discards,
                ..
            } => {
                let result = machine
                    .result
                    .structural()
                    .expect("control validation requires a structural result");
                let source_signature = machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == *source)
                    .map(|parameter| {
                        (
                            parameter.structural_type,
                            parameter.multiplicity,
                            parameter.qualifications.as_slice(),
                        )
                    })
                    .or_else(|| {
                        machine
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .find_map(|operation| {
                                operation.result.structural().and_then(|source_result| {
                                    (source_result.place == *source).then_some((
                                        source_result.structural_type,
                                        source_result.multiplicity,
                                        source_result.qualifications.as_slice(),
                                    ))
                                })
                            })
                    })
                    .expect("control validation requires a structural source declaration");
                if frontier.owned_places.remove(source).is_none() {
                    return Err(ModuleError::StructuralReturnSourceNotLive {
                        machine: machine.id,
                        block: block.id,
                        place: *source,
                    });
                }
                if source_signature.0 != result.structural_type
                    || source_signature.1 != result.multiplicity
                    || source_signature.2 != result.qualifications
                {
                    return Err(ModuleError::StructuralReturnSignatureMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                if returned_claims.is_empty()
                    || returned_claims.windows(2).any(|pair| pair[0] >= pair[1])
                {
                    return Err(ModuleError::NonCanonicalStructuralReturnClaims {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                let expected_claims = frontier
                    .claims
                    .iter()
                    .filter_map(|(claim, live)| (live.input == Some(*source)).then_some(*claim))
                    .collect::<Vec<_>>();
                if *returned_claims != expected_claims {
                    return Err(ModuleError::StructuralReturnClaimSetMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                for claim in returned_claims {
                    frontier.claims.remove(claim);
                }
                let expected_affine_discards = expected_trivial_affine_discards(machine, &frontier);
                if *trivial_affine_discards != expected_affine_discards {
                    return Err(ModuleError::StructuralReturnAffineDiscardsMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                if let Some(claim) = frontier.claims.keys().next() {
                    return Err(ModuleError::LiveClaimAtStructuralReturn {
                        machine: machine.id,
                        block: block.id,
                        claim: *claim,
                    });
                }
            }
            Terminator::Crash {
                frontier_lower_bound,
                ..
            } => {
                let expected = frontier.claims.keys().copied().collect::<Vec<_>>();
                if frontier_lower_bound != &expected {
                    return Err(ModuleError::CrashFrontierMismatch { block: block.id });
                }
            }
        }
    }
    Ok(())
}

fn validate_scalar_cleanup_actions(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    block: BlockId,
    frontier: &StructuralOwnershipFrontier,
    actions: &[TerminalAffineCleanupAction],
) -> Result<(), ModuleError> {
    let mismatch = || ModuleError::ScalarReturnAffineDiscardsMismatch {
        machine: machine.id,
        block,
    };
    let mut frontier = frontier.clone();
    let mut actions = actions.iter();

    let mut locals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if frontier.owned_places.contains_key(&place.id) => {
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
        frontier.owned_places.remove(&place);
    }

    for parameter in machine.structural_parameters.iter().rev() {
        if !frontier.owned_places.contains_key(&parameter.place) {
            continue;
        }
        if parameter.multiplicity != StructuralMultiplicity::Affine
            || frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(parameter.place))
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == parameter.place)
        {
            return Err(mismatch());
        }
        if let Some(moved) = frontier.moved_field_paths.remove(&parameter.place) {
            let Some(residuals) =
                partial_affine_residuals(module, parameter.structural_type, &moved)
            else {
                return Err(mismatch());
            };
            if moved.is_empty() || residuals.is_empty() {
                return Err(mismatch());
            }
            for (path, structural_type) in residuals {
                let expected = TerminalAffineCleanupAction::DiscardResidual(
                    psi_terminal::StructuralAffineDiscard {
                        place: parameter.place,
                        path,
                        structural_type,
                    },
                );
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
                        && valid_scalar_nominal_cleanup(module, machine, machines, cleanup) => {}
                _ => return Err(mismatch()),
            }
        }
        frontier.owned_places.remove(&parameter.place);
    }

    if actions.next().is_some()
        || !frontier.owned_places.is_empty()
        || !frontier.moved_field_paths.is_empty()
    {
        return Err(mismatch());
    }
    Ok(())
}

fn valid_scalar_nominal_cleanup(
    module: &TerminalModule,
    caller: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    cleanup: &psi_terminal::NominalAffineCleanup,
) -> bool {
    let Some(source) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanup.structural_type)
    else {
        return false;
    };
    let Some(target) = machines.get(&cleanup.cleanup_machine).copied() else {
        return false;
    };
    cleanup.cleanup_machine != caller.id
        && bounded_nominal_cleanup_receiver_shape(&source.shape)
        && target.attachment == Some(cleanup.structural_type)
        && target.result == TerminalMachineResult::Unit
        && target.parameters.is_empty()
        && target.structural_parameters.is_empty()
        && target.entry_claims.is_empty()
        && target.content_entry_claims.is_empty()
        && target.contract.ensures.is_empty()
        && target.contract.crash_routes.is_empty()
        && cleanup.requirement_obligations.len() == target.contract.requires.len()
        && valid_nominal_cleanup_requirements(module, target, cleanup)
}

fn expected_trivial_affine_discards(
    machine: &TerminalMachine,
    frontier: &StructuralOwnershipFrontier,
) -> Vec<PlaceId> {
    let mut output = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
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
        machine
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
                    && !machine
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == parameter.place))
                .then_some(parameter.place)
            })
            .collect::<Vec<_>>(),
    );
    output
}

fn apply_edge_trivial_affine_discards(
    machine: &TerminalMachine,
    frontier: &mut StructuralOwnershipFrontier,
    edge: EdgeId,
    discards: &[PlaceId],
) -> Result<(), ModuleError> {
    let eligible = expected_trivial_affine_discards(machine, frontier);
    let mut next = 0;
    for eligible_place in eligible {
        if discards.get(next) == Some(&eligible_place) {
            next += 1;
        }
    }
    if next != discards.len() {
        return Err(ModuleError::EdgeAffineDiscardsInvalid { edge });
    }
    for place in discards {
        frontier.owned_places.remove(place);
    }
    Ok(())
}
