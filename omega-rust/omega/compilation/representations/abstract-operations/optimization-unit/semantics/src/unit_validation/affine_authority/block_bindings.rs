//! Rejoin retained edge snapshots after cleanup and whole owned rebinding.
use crate::unit_validation::affine_authority::valid_edge_partial_affine_transition;
use abstract_operations::AbstractStructuralBinding;
use optimization_unit::{
    OwnershipFrontierOwnedPlace, OwnershipFrontierSnapshot, PsiOptimizationFunction,
};
use semantic_vocabulary::PlaceId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

pub(super) fn valid_transition(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<
        semantic_vocabulary::StructuralTypeId,
        &terminal_psi::StructuralTypeDeclaration,
    >,
    entry: &OwnershipFrontierSnapshot,
    exit: &OwnershipFrontierSnapshot,
    discards: &[PlaceId],
    residuals: &[terminal_psi::StructuralAffineDiscard],
    bindings: &[AbstractStructuralBinding],
) -> bool {
    if bindings.is_empty() {
        return valid_edge_partial_affine_transition(
            function, types, entry, exit, discards, residuals,
        );
    }
    let mut expected = entry.clone();
    expected.owned_places.retain(|owned| {
        !discards.contains(&owned.place)
            && !residuals.iter().any(|discard| discard.place == owned.place)
    });
    expected.partial_custody.retain(|custody| {
        !residuals
            .iter()
            .any(|discard| discard.place == custody.place)
    });
    if !valid_edge_partial_affine_transition(function, types, entry, &expected, discards, residuals)
    {
        return false;
    }
    let mut owned = expected
        .owned_places
        .iter()
        .map(|owned| (owned.place, owned.multiplicity))
        .collect::<BTreeMap<_, _>>();
    let parameters = function
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters);
    let mut arrivals = Vec::new();
    let mut transferred_claims = Vec::new();
    let mut removed_claims = BTreeSet::new();
    // Signature replay checks each exact source/destination type, access and
    // multiplicity. Read claim custody from the immutable entry snapshot, then
    // remove every source before installing any arrival, preserving swaps and
    // self-loops without treating a new destination as another old source.
    for binding in bindings {
        let Some(parameter) = parameters
            .clone()
            .find(|parameter| parameter.place == binding.parameter)
        else {
            return false;
        };
        if parameter.access != StructuralAccess::Owned {
            continue;
        }
        let source = binding.argument.place;
        if !binding.argument.path.is_empty()
            || binding.argument.access != StructuralAccess::Owned
            || expected
                .partial_custody
                .iter()
                .any(|custody| custody.place == source)
        {
            return false;
        }
        let source_claims = entry
            .claims
            .iter()
            .filter(|claim| claim.input == Some(source))
            .collect::<Vec<_>>();
        if (parameter.multiplicity == StructuralMultiplicity::Linear && source_claims.is_empty())
            || (parameter.multiplicity != StructuralMultiplicity::Linear
                && !source_claims.is_empty())
        {
            return false;
        }
        if parameter.multiplicity != StructuralMultiplicity::Unrestricted {
            if owned.remove(&source) != Some(parameter.multiplicity) {
                return false;
            }
            arrivals.push(parameter);
        }
        for claim in source_claims {
            if !removed_claims.insert(claim.claim) {
                return false;
            }
            let mut moved = claim.clone();
            moved.input = Some(parameter.place);
            transferred_claims.push(moved);
        }
    }
    expected
        .claims
        .retain(|claim| !removed_claims.contains(&claim.claim));
    for parameter in arrivals {
        if owned
            .insert(parameter.place, parameter.multiplicity)
            .is_some()
            || expected
                .claims
                .iter()
                .any(|claim| claim.input == Some(parameter.place))
        {
            return false;
        }
    }
    for claim in transferred_claims {
        if expected
            .claims
            .iter()
            .any(|current| current.claim == claim.claim)
        {
            return false;
        }
        expected.claims.push(claim);
    }
    expected.claims.sort_by_key(|claim| claim.claim);
    expected.owned_places = owned
        .into_iter()
        .map(|(place, multiplicity)| OwnershipFrontierOwnedPlace {
            place,
            multiplicity,
        })
        .collect();
    expected == *exit
}
