//! Retained edge-cleanup and hidden-establishment affine authority.

use super::*;

pub(super) fn validate_retained_ownership_authority(
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
