//! Reconstructible optimization-unit validation and affine custody checks.

use super::*;

pub fn validate_psi_optimization_unit(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    let recomputed = recompute_psi_optimization_unit_identity(unit);
    if unit.identity != recomputed {
        return Err(OptimizationUnitValidationError::ContentIdentityMismatch {
            stored: unit.identity,
            recomputed,
        });
    }
    if unit.fuel_schedule != TerminalFuelSchedule::CURRENT.identity() {
        return Err(OptimizationUnitValidationError::WrongFuelSchedule);
    }
    if unit
        .accepted_obligation_facts
        .iter()
        .any(|fact| fact.psi != unit.psi || !fact.has_canonical_identity())
        || unit.accepted_obligation_facts.windows(2).any(|pair| {
            (pair[0].machine, pair[0].operation, pair[0].obligation)
                >= (pair[1].machine, pair[1].operation, pair[1].obligation)
        })
    {
        return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
    }
    let mut proof_question_identities = BTreeSet::new();
    let mut proof_question_owners = BTreeSet::new();
    if unit.proof_questions.iter().any(|question| {
        question.terminal_psi != unit.psi
            || !question.has_canonical_identity()
            || !proof_question_identities.insert(question.identity)
            || !proof_question_owners.insert((question.owner, question.obligation))
    }) {
        return Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch);
    }
    if unit.ownership_frontier_facts.iter().any(|fact| {
        fact.psi != unit.psi
            || !fact.has_canonical_identity()
            || !canonical_ownership_frontier_snapshot(&fact.snapshot)
    }) || unit
        .ownership_frontier_facts
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].site) >= (pair[1].machine, pair[1].site))
    {
        return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
    }
    let mut machines = BTreeMap::new();
    for function in &unit.functions {
        if machines.insert(function.machine, function).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateMachine(
                function.machine,
            ));
        }
    }
    if unit
        .pruned_machines
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(OptimizationUnitValidationError::NonCanonicalPrunedMachineRoster);
    }
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|custody| custody.machine)
        .collect::<BTreeSet<_>>();
    if pruned.len() != unit.pruned_machines.len() {
        return Err(OptimizationUnitValidationError::NonCanonicalPrunedMachineRoster);
    }
    if let Some(machine) = machines
        .keys()
        .find(|machine| pruned.contains(machine))
        .copied()
    {
        return Err(OptimizationUnitValidationError::ActivePrunedMachineOverlap(
            machine,
        ));
    }
    if pruned.contains(&unit.entry) {
        return Err(OptimizationUnitValidationError::PrunedEntryMachine(
            unit.entry,
        ));
    }
    if let Some(machine) = unit
        .provider_candidates
        .iter()
        .map(|candidate| candidate.candidate)
        .find(|machine| pruned.contains(machine))
    {
        return Err(OptimizationUnitValidationError::PrunedProviderMachine(
            machine,
        ));
    }
    if unit
        .accepted_obligation_facts
        .iter()
        .any(|fact| !machines.contains_key(&fact.machine) && !pruned.contains(&fact.machine))
    {
        return Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch);
    }
    if unit.proof_questions.iter().any(|question| {
        let machine = question.owner.machine();
        !machines.contains_key(&machine) && !pruned.contains(&machine)
    }) {
        return Err(OptimizationUnitValidationError::ProofQuestionIndexMismatch);
    }
    let mut boundary_machines = BTreeMap::new();
    for boundary in &unit.boundary_machines {
        if boundary_machines.insert(boundary.id, boundary).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateBoundaryMachine(
                boundary.id,
            ));
        }
    }
    let services = index_service_catalog(unit)?;
    let (structural_types, structural_domains) = index_structural_catalogs(unit)?;
    for boundary in &unit.boundary_machines {
        if !valid_service_ceiling(&boundary.published_service_ceiling, &services) {
            return Err(
                OptimizationUnitValidationError::InvalidBoundaryServiceCeiling(boundary.id),
            );
        }
        if !boundary_structural_signature_matches(boundary, &structural_types, &structural_domains)
        {
            return Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                machine: None,
            });
        }
    }
    validate_provider_service_refinements(unit, &machines, &boundary_machines)?;
    for function in &unit.functions {
        validate_function(
            function,
            unit.entry,
            &machines,
            &boundary_machines,
            &services,
            &structural_types,
            &structural_domains,
        )?;
    }
    validate_retained_ownership_authority(unit)?;
    for fact in &unit.ownership_frontier_facts {
        if unit
            .functions
            .iter()
            .find(|function| function.machine == fact.machine)
            .is_none()
            && !pruned.contains(&fact.machine)
        {
            return Err(OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch);
        }
    }
    if !machines.contains_key(&unit.entry) {
        return Err(OptimizationUnitValidationError::MissingEntryMachine(
            unit.entry,
        ));
    }
    validate_root_service_reach(unit, &machines, &boundary_machines, &services)?;
    Ok(())
}

/// Validate the bounded ownership authority retained by the current unit.
///
/// This intentionally does not replay the current CFG ownership automaton.
/// It binds authored edge cleanup and compressed hidden establishments to the
/// immutable source-site entry/exit transitions that remain in the unit.
pub(crate) fn validate_retained_ownership_authority(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    if unit.ownership_frontier_facts.is_empty() {
        // Bare reconstruction seeds have no verifier authority to replay.
        return Ok(());
    }
    let frontiers = unit
        .ownership_frontier_facts
        .iter()
        .map(|fact| ((fact.machine, fact.site), &fact.snapshot))
        .collect::<BTreeMap<_, _>>();

    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let node_index = u32::try_from(node_index).expect("unit node index fits u32");
                for edge in &node.successors {
                    for (source_index, source) in edge.provenance.iter().enumerate() {
                        let PsiProvenance::Edge(source) = source else {
                            return Err(
                                OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch {
                                    machine: function.machine,
                                    edge: edge.psi_edge,
                                },
                            );
                        };
                        let Some(entry) = frontiers
                            .get(&(function.machine, OwnershipFrontierSite::EdgeEntry(*source)))
                        else {
                            return Err(
                                OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                                    machine: function.machine,
                                    edge: *source,
                                },
                            );
                        };
                        let Some(exit) = frontiers
                            .get(&(function.machine, OwnershipFrontierSite::EdgeExit(*source)))
                        else {
                            return Err(
                                OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                                    machine: function.machine,
                                    edge: *source,
                                },
                            );
                        };
                        let discards = if source_index == 0 {
                            edge.trivial_affine_discards.as_slice()
                        } else {
                            // Every implemented edge-combining rewrite fences
                            // nonempty inherited cleanup work.
                            &[]
                        };
                        if !valid_edge_affine_transition(function, entry, exit, discards) {
                            return Err(
                                OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch {
                                    machine: function.machine,
                                    edge: edge.psi_edge,
                                },
                            );
                        }
                    }
                }

                let O::ReturnStructural {
                    trivial_affine_locals,
                    ..
                } = &node.operation
                else {
                    continue;
                };
                for (operation, place, _) in trivial_affine_locals {
                    let mismatch = || {
                        OptimizationUnitValidationError::StructuralReturnHiddenLocalCustodyMismatch {
                            machine: function.machine,
                            block: block.id,
                            node: node_index,
                            operation: *operation,
                        }
                    };
                    let entry = frontiers
                        .get(&(
                            function.machine,
                            OwnershipFrontierSite::OperationEntry(*operation),
                        ))
                        .ok_or_else(mismatch)?;
                    let exit = frontiers
                        .get(&(
                            function.machine,
                            OwnershipFrontierSite::OperationExit(*operation),
                        ))
                        .ok_or_else(mismatch)?;
                    if !valid_hidden_affine_establishment(entry, exit, place.id) {
                        return Err(mismatch());
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn valid_edge_affine_transition(
    function: &PsiOptimizationFunction,
    entry: &OwnershipFrontierSnapshot,
    exit: &OwnershipFrontierSnapshot,
    discards: &[PlaceId],
) -> bool {
    if entry.claims != exit.claims || entry.partial_custody != exit.partial_custody {
        return false;
    }
    let live = entry
        .owned_places
        .iter()
        .map(|owned| owned.place)
        .collect::<BTreeSet<_>>();
    let mut eligible = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if live.contains(&place.id) => Some((declaration_ordinal, place.id)),
            _ => None,
        })
        .collect::<Vec<_>>();
    eligible.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    let mut eligible = eligible
        .into_iter()
        .map(|(_, place)| place)
        .collect::<Vec<_>>();
    eligible.extend(
        function
            .structural_parameters
            .iter()
            .rev()
            .filter_map(|parameter| {
                (parameter.multiplicity == psi_terminal::StructuralMultiplicity::Affine
                    && live.contains(&parameter.place)
                    && !entry
                        .claims
                        .iter()
                        .any(|claim| claim.input == Some(parameter.place))
                    && !function
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == parameter.place))
                .then_some(parameter.place)
            }),
    );
    let mut next = 0;
    for eligible_place in eligible {
        if discards.get(next) == Some(&eligible_place) {
            next += 1;
        }
    }
    if next != discards.len() {
        return false;
    }
    let discard_set = discards.iter().copied().collect::<BTreeSet<_>>();
    if discard_set.len() != discards.len() {
        return false;
    }
    let expected_exit = entry
        .owned_places
        .iter()
        .filter(|owned| !discard_set.contains(&owned.place))
        .copied()
        .collect::<Vec<_>>();
    expected_exit == exit.owned_places
}

pub(crate) fn valid_hidden_affine_establishment(
    entry: &OwnershipFrontierSnapshot,
    exit: &OwnershipFrontierSnapshot,
    place: PlaceId,
) -> bool {
    let mut expected_owned = entry.owned_places.clone();
    if expected_owned.iter().any(|owned| owned.place == place) {
        return false;
    }
    expected_owned.push(OwnershipFrontierOwnedPlace {
        place,
        multiplicity: psi_terminal::StructuralMultiplicity::Affine,
    });
    expected_owned.sort_by_key(|owned| owned.place);
    entry.claims == exit.claims
        && entry.partial_custody == exit.partial_custody
        && expected_owned == exit.owned_places
}
