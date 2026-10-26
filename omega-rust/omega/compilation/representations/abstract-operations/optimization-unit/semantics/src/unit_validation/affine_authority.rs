//! Retained edge-cleanup and hidden-establishment affine authority.
use crate::{OperationSite, OptimizationUnitValidationError};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    OwnershipFrontierOwnedPlace, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    PsiOptimizationUnit, PsiProvenance,
};
use semantic_vocabulary::PlaceId;
use std::collections::BTreeMap;

mod block_bindings;

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
    let structural_types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
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
                        // Every implemented edge-combining rewrite fences
                        // nonempty inherited cleanup work and bindings.
                        let (actions, bindings) = if source_index == 0 {
                            (
                                edge.cleanup_actions.as_slice(),
                                edge.structural_bindings.as_slice(),
                            )
                        } else {
                            (&[][..], &[][..])
                        };
                        if !block_bindings::valid_transition(
                            function,
                            &structural_types,
                            entry,
                            exit,
                            actions,
                            bindings,
                        ) {
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
                        OptimizationUnitValidationError::StructuralReturnHiddenLocalCustodyMismatch(
                            OperationSite::new(function.machine, block.id, node_index),
                            *operation,
                        )
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
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
    });
    expected_owned.sort_by_key(|owned| owned.place);
    entry.claims == exit.claims
        && entry.partial_custody == exit.partial_custody
        && expected_owned == exit.owned_places
}
