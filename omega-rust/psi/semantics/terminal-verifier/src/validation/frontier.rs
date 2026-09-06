use super::affine_cleanup::{
    bounded_nominal_cleanup_receiver_shape, valid_nominal_cleanup_requirements,
};
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedLiveClaim {
    pub claim: ClaimId,
    pub input: Option<PlaceId>,
    pub path: Vec<StructuralPathSegment>,
    pub multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedOwnedStructuralPlace {
    pub place: PlaceId,
    pub multiplicity: StructuralMultiplicity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPartialStructuralCustody {
    pub place: PlaceId,
    pub moved_paths: Vec<Vec<StructuralPathSegment>>,
}

/// Exact verifier-owned ownership state at one deterministic control site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedStructuralOwnershipFrontier {
    claims: Vec<VerifiedLiveClaim>,
    owned_places: Vec<VerifiedOwnedStructuralPlace>,
    partial_custody: Vec<VerifiedPartialStructuralCustody>,
}

impl VerifiedStructuralOwnershipFrontier {
    pub fn claims(&self) -> &[VerifiedLiveClaim] {
        &self.claims
    }

    pub fn owned_places(&self) -> &[VerifiedOwnedStructuralPlace] {
        &self.owned_places
    }

    pub fn partial_custody(&self) -> &[VerifiedPartialStructuralCustody] {
        &self.partial_custody
    }
}

/// Path-sensitive frontier snapshots for one verified Terminal-Psi machine.
/// Entries and exits are separately retained so a rewrite cannot treat a
/// transfer as a timeless membership fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedMachineStructuralFrontiers {
    pub machine: MachineId,
    block_entries: BTreeMap<BlockId, VerifiedStructuralOwnershipFrontier>,
    operation_entries: BTreeMap<OperationId, VerifiedStructuralOwnershipFrontier>,
    operation_exits: BTreeMap<OperationId, VerifiedStructuralOwnershipFrontier>,
    edge_entries: BTreeMap<EdgeId, VerifiedStructuralOwnershipFrontier>,
    edge_exits: BTreeMap<EdgeId, VerifiedStructuralOwnershipFrontier>,
}

impl VerifiedMachineStructuralFrontiers {
    pub fn block_entry(&self, block: BlockId) -> Option<&VerifiedStructuralOwnershipFrontier> {
        self.block_entries.get(&block)
    }

    pub fn operation_entry(
        &self,
        operation: OperationId,
    ) -> Option<&VerifiedStructuralOwnershipFrontier> {
        self.operation_entries.get(&operation)
    }

    pub fn operation_exit(
        &self,
        operation: OperationId,
    ) -> Option<&VerifiedStructuralOwnershipFrontier> {
        self.operation_exits.get(&operation)
    }

    pub fn edge_exit(&self, edge: EdgeId) -> Option<&VerifiedStructuralOwnershipFrontier> {
        self.edge_exits.get(&edge)
    }

    pub fn edge_entry(&self, edge: EdgeId) -> Option<&VerifiedStructuralOwnershipFrontier> {
        self.edge_entries.get(&edge)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTerminalStructuralFrontiers {
    pub(super) machines: Vec<VerifiedMachineStructuralFrontiers>,
}

impl VerifiedTerminalStructuralFrontiers {
    pub fn machines(&self) -> &[VerifiedMachineStructuralFrontiers] {
        &self.machines
    }

    pub fn machine(&self, machine: MachineId) -> Option<&VerifiedMachineStructuralFrontiers> {
        self.machines
            .iter()
            .find(|candidate| candidate.machine == machine)
    }
}

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
    /// Exact projected paths already transferred from an otherwise-live owned
    /// root. This is independent of the root's multiplicity: no whole-root use
    /// is legal while a hole remains. Affine roots close through explicit
    /// residual cleanup; a fixed-array root closes only after its complete
    /// dense sibling set has moved. Linear arrays retain their existing general
    /// rule, while affine arrays admit only the exact two-element/no-residual
    /// carrier so no cleanup order is inferred.
    partial_custody_paths: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>>,
}

impl StructuralOwnershipFrontier {
    fn snapshot(&self) -> VerifiedStructuralOwnershipFrontier {
        VerifiedStructuralOwnershipFrontier {
            claims: self
                .claims
                .iter()
                .map(|(claim, live)| VerifiedLiveClaim {
                    claim: *claim,
                    input: live.input,
                    path: live.path.clone(),
                    multiplicity: live.multiplicity,
                })
                .collect(),
            owned_places: self
                .owned_places
                .iter()
                .map(|(place, multiplicity)| VerifiedOwnedStructuralPlace {
                    place: *place,
                    multiplicity: *multiplicity,
                })
                .collect(),
            partial_custody: self
                .partial_custody_paths
                .iter()
                .map(|(place, moved_paths)| VerifiedPartialStructuralCustody {
                    place: *place,
                    moved_paths: moved_paths.iter().cloned().collect(),
                })
                .collect(),
        }
    }
}

pub(super) fn validate_structural_frontier(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    representation_backedges: &BTreeSet<EdgeId>,
) -> Result<VerifiedMachineStructuralFrontiers, ModuleError> {
    let mut snapshots = VerifiedMachineStructuralFrontiers {
        machine: machine.id,
        block_entries: BTreeMap::new(),
        operation_entries: BTreeMap::new(),
        operation_exits: BTreeMap::new(),
        edge_entries: BTreeMap::new(),
        edge_exits: BTreeMap::new(),
    };
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
                // Access attenuations on ordinary by-value parameters do not
                // surrender custody. A borrowed receiver is the explicit
                // exception: `self` remains in the signature but never enters
                // the machine's by-value ownership frontier.
                (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    && !(parameter.is_self && parameter.access != StructuralAccess::Owned))
                    .then_some((parameter.place, parameter.multiplicity))
            })
            .collect(),
        partial_custody_paths: BTreeMap::new(),
    };

    let mut successors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut predecessors = blocks
        .keys()
        .map(|block| (*block, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for block in blocks.values() {
        let targets = match &block.terminator {
            Terminator::Jump { edge, target, .. } => vec![(*edge, *target)],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![
                (when_true.edge, when_true.target),
                (when_false.edge, when_false.target),
            ],
            Terminator::StructuralCase { cases, .. } => {
                cases.iter().map(|case| (case.edge, case.target)).collect()
            }
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        let targets = targets
            .into_iter()
            .filter_map(|(edge, target)| {
                (!representation_backedges.contains(&edge)).then_some(target)
            })
            .collect::<Vec<_>>();
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
                .any(|candidate| candidate.partial_custody_paths != frontier.partial_custody_paths)
        {
            return Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block_id));
        }
        let block = blocks
            .get(&block_id)
            .expect("topological order contains known blocks");
        let mut frontier = frontier;
        snapshots
            .block_entries
            .insert(block.id, frontier.snapshot());
        for operation in &block.operations {
            snapshots
                .operation_entries
                .insert(operation.id, frontier.snapshot());
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
            let claims = match &operation.kind {
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
            };
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
                | OperationKind::CallStructuralWithScalarArguments {
                    structural_arguments,
                    ..
                }
                | OperationKind::BoundaryCall {
                    structural_arguments,
                    ..
                } => structural_arguments.as_slice(),
                _ => &[],
            };
            for argument in projected_arguments.iter().filter(|argument| {
                !argument.path.is_empty() && argument.access == StructuralAccess::Owned
            }) {
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
                if projected_fixed_array_root_is_fully_consumed(
                    module,
                    machine,
                    &frontier,
                    argument.place,
                ) {
                    frontier.owned_places.remove(&argument.place);
                    frontier.partial_custody_paths.remove(&argument.place);
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
            snapshots
                .operation_exits
                .insert(operation.id, frontier.snapshot());
        }
        for edge in block.terminator.edges() {
            snapshots.edge_entries.insert(edge, frontier.snapshot());
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
                snapshots.edge_exits.insert(*edge, frontier.snapshot());
                if !representation_backedges.contains(edge) {
                    incoming.entry(*target).or_default().push(frontier);
                }
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
                snapshots
                    .edge_exits
                    .insert(when_true.edge, true_frontier.snapshot());
                if !representation_backedges.contains(&when_true.edge) {
                    incoming
                        .entry(when_true.target)
                        .or_default()
                        .push(true_frontier);
                }
                apply_edge_trivial_affine_discards(
                    machine,
                    &mut frontier,
                    when_false.edge,
                    &when_false.trivial_affine_discards,
                )?;
                snapshots
                    .edge_exits
                    .insert(when_false.edge, frontier.snapshot());
                if !representation_backedges.contains(&when_false.edge) {
                    incoming
                        .entry(when_false.target)
                        .or_default()
                        .push(frontier);
                }
            }
            Terminator::StructuralCase { cases, .. } => {
                for case in cases {
                    let mut case_frontier = frontier.clone();
                    apply_edge_trivial_affine_discards(
                        machine,
                        &mut case_frontier,
                        case.edge,
                        &case.trivial_affine_discards,
                    )?;
                    snapshots
                        .edge_exits
                        .insert(case.edge, case_frontier.snapshot());
                    if !representation_backedges.contains(&case.edge) {
                        incoming.entry(case.target).or_default().push(case_frontier);
                    }
                }
            }
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => {
                if let Some(place) = frontier.partial_custody_paths.keys().next() {
                    return Err(ModuleError::PartialStructuralCustodyAtUnitReturn {
                        machine: machine.id,
                        block: block.id,
                        place: *place,
                    });
                }
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
                let Some(moved) = frontier.partial_custody_paths.remove(&root_place) else {
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
                if !frontier.partial_custody_paths.is_empty() {
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
                if !frontier.partial_custody_paths.is_empty()
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
                if frontier.partial_custody_paths.contains_key(source) {
                    return Err(ModuleError::StructuralReturnSourcePartiallyMoved {
                        machine: machine.id,
                        block: block.id,
                        place: *source,
                    });
                }
                let result = machine
                    .result
                    .structural()
                    .expect("control validation requires a structural result");
                let source_signature =
                    super::structural_result_contracts::source_signature(machine, *source)
                        .expect("control validation requires a structural source declaration");
                let exact_unrestricted_parameter_return = source_signature.multiplicity
                    == StructuralMultiplicity::Unrestricted
                    && super::structural_result_contracts::has_empty_qualification_rosters(
                        source_signature.qualifications,
                        source_signature.projected_qualifications,
                    )
                    && matches!(machine.structural_parameters.as_slice(), [parameter]
                        if parameter.place == *source
                            && parameter.position == 0
                            && !parameter.is_self
                            && parameter.access == StructuralAccess::Owned);
                if frontier.owned_places.remove(source).is_none()
                    && !exact_unrestricted_parameter_return
                {
                    return Err(ModuleError::StructuralReturnSourceNotLive {
                        machine: machine.id,
                        block: block.id,
                        place: *source,
                    });
                }
                if !super::structural_result_contracts::matches_function_result(
                    source_signature,
                    result,
                ) {
                    return Err(ModuleError::StructuralReturnSignatureMismatch {
                        machine: machine.id,
                        block: block.id,
                    });
                }
                let exact_payloadless_claim_free_return = returned_claims.is_empty()
                    && source_signature.multiplicity == StructuralMultiplicity::Unrestricted
                    && super::structural_result_contracts::has_empty_qualification_rosters(
                        source_signature.qualifications,
                        source_signature.projected_qualifications,
                    )
                    && machine
                        .structural_places
                        .iter()
                        .find(|place| place.id == *source)
                        .and_then(|place| match place.kind {
                            StructuralPlaceKind::OperationResult { producer, .. } => machine
                                .blocks
                                .iter()
                                .flat_map(|block| &block.operations)
                                .find(|operation| operation.id == producer),
                            _ => None,
                        })
                        .is_some_and(|operation| {
                            (matches!(
                                operation.kind,
                                OperationKind::EstablishPayloadlessCase { .. }
                            ) || super::structural_operations::exact_payloadless_structural_call(
                                module, operation, machines,
                            )) && operation.result.structural().is_some_and(|result| {
                                result.place == *source
                                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                                    && result.qualifications.is_empty()
                                    && result.projected_qualifications.is_empty()
                                    && result.claims.is_empty()
                            })
                        })
                    || exact_unrestricted_parameter_return;
                let exact_affine_parameter_return = returned_claims.is_empty()
                    && source_signature.multiplicity == StructuralMultiplicity::Affine
                    && super::structural_result_contracts::has_empty_qualification_rosters(
                        source_signature.qualifications,
                        source_signature.projected_qualifications,
                    )
                    && machine.parameters.iter().all(|parameter| {
                        matches!(
                            parameter.scalar_type,
                            ScalarType::Integer(integer)
                                if matches!(integer.bits(), 8 | 16 | 32 | 64)
                        )
                    })
                    && matches!(machine.structural_parameters.as_slice(), [parameter]
                        if parameter.place == *source
                            && parameter.position == 0
                            && !parameter.is_self
                            && parameter.multiplicity == StructuralMultiplicity::Affine
                            && parameter.access == StructuralAccess::Owned
                            && parameter.qualifications.is_empty()
                            && parameter.projected_qualifications.is_empty())
                    && machine.entry_claims.is_empty()
                    && machine.content_entry_claims.is_empty()
                    && machine.published_service_ceiling.is_empty()
                    && machine.contract.crash_routes.is_empty()
                    && machine.contract.requires.is_empty()
                    && machine.contract.ensures.is_empty()
                    && machine.contract.outcome_specific_ensures.is_empty()
                    && matches!(machine.blocks.as_slice(), [only]
                        if only.id == machine.entry && only.id == block.id
                            && only.parameters.is_empty()
                            && only.operations.is_empty())
                    && module.structural_types.iter().any(|declaration| {
                        declaration.id == result.structural_type
                            && matches!(
                                &declaration.shape,
                                StructuralTypeShape::Record { fields }
                                    if matches!(fields.as_slice(), [field]
                                        if matches!(
                                            field.field_type,
                                            StructuralFieldType::Scalar(
                                                ScalarType::Integer(integer)
                                            ) if integer.bits() == 64
                                        ))
                            )
                    });
                if (returned_claims.is_empty()
                    && !exact_payloadless_claim_free_return
                    && !exact_affine_parameter_return)
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
    if let Some(component) = &machine.ranked_scc {
        for row in &component.covered_cyclic_edges {
            let established = snapshots
                .block_entries
                .get(&row.target)
                .expect("ranked validation established the backedge target block");
            let preserved = snapshots
                .edge_exits
                .get(&row.edge)
                .expect("frontier replay visited the ranked backedge");
            require_snapshot_match(row.target, established, preserved)?;
        }
    }
    Ok(snapshots)
}

fn require_snapshot_match(
    block: BlockId,
    expected: &VerifiedStructuralOwnershipFrontier,
    candidate: &VerifiedStructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    if candidate.claims != expected.claims {
        return Err(ModuleError::ClaimFrontierJoinMismatch(block));
    }
    if candidate.owned_places != expected.owned_places
        || candidate.partial_custody != expected.partial_custody
    {
        return Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block));
    }
    Ok(())
}

fn projected_fixed_array_root_is_fully_consumed(
    module: &TerminalModule,
    machine: &TerminalMachine,
    frontier: &StructuralOwnershipFrontier,
    place: PlaceId,
) -> bool {
    let Some(parameter) = machine
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
    let Some(StructuralTypeShape::FixedArray { element, length }) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)
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
                module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == *element)
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
        if let Some(moved) = frontier.partial_custody_paths.remove(&parameter.place) {
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
                    terminal_psi::StructuralAffineDiscard {
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
        || !frontier.partial_custody_paths.is_empty()
    {
        return Err(mismatch());
    }
    Ok(())
}

fn valid_scalar_nominal_cleanup(
    module: &TerminalModule,
    caller: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    cleanup: &terminal_psi::NominalAffineCleanup,
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
    let mut operation_results = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::OperationResult { producer, .. }
                if frontier.owned_places.get(&place.id)
                    == Some(&StructuralMultiplicity::Affine)
                    && !frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(place.id)) =>
            {
                Some((producer, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    operation_results.sort_by_key(|(producer, _)| std::cmp::Reverse(*producer));
    let mut output = operation_results
        .into_iter()
        .map(|(_, place)| place)
        .collect::<Vec<_>>();
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
    output.extend(locals.into_iter().map(|(_, place)| place));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_snapshot() -> VerifiedStructuralOwnershipFrontier {
        VerifiedStructuralOwnershipFrontier {
            claims: Vec::new(),
            owned_places: Vec::new(),
            partial_custody: Vec::new(),
        }
    }

    #[test]
    fn ranked_preservation_compares_every_frontier_axis_in_diagnostic_order() {
        let block = BlockId::new(1).unwrap();
        let expected = empty_snapshot();
        assert_eq!(require_snapshot_match(block, &expected, &expected), Ok(()));

        let mut claim_drift = empty_snapshot();
        claim_drift.claims.push(VerifiedLiveClaim {
            claim: ClaimId::new(1).unwrap(),
            input: None,
            path: Vec::new(),
            multiplicity: None,
        });
        assert_eq!(
            require_snapshot_match(block, &expected, &claim_drift),
            Err(ModuleError::ClaimFrontierJoinMismatch(block))
        );

        let place = PlaceId::new(1).unwrap();
        let mut owned_drift = empty_snapshot();
        owned_drift.owned_places.push(VerifiedOwnedStructuralPlace {
            place,
            multiplicity: StructuralMultiplicity::Affine,
        });
        assert_eq!(
            require_snapshot_match(block, &expected, &owned_drift),
            Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block))
        );

        let mut partial_drift = empty_snapshot();
        partial_drift
            .partial_custody
            .push(VerifiedPartialStructuralCustody {
                place,
                moved_paths: vec![vec![StructuralPathSegment::FixedIndex(0)]],
            });
        assert_eq!(
            require_snapshot_match(block, &expected, &partial_drift),
            Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block))
        );
    }
}
