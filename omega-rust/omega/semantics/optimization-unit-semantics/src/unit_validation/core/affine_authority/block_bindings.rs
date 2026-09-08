//! Rejoin retained edge snapshots after cleanup and whole owned rebinding.

use super::*;
use abstract_operations::AbstractStructuralBinding;
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
                .claims
                .iter()
                .any(|claim| claim.input == Some(source))
            || expected
                .partial_custody
                .iter()
                .any(|custody| custody.place == source)
        {
            return false;
        }
        if parameter.multiplicity == StructuralMultiplicity::Affine {
            if owned.remove(&source) != Some(StructuralMultiplicity::Affine) {
                return false;
            }
            arrivals.push(parameter);
        }
    }
    for parameter in arrivals {
        if owned
            .insert(parameter.place, parameter.multiplicity)
            .is_some()
        {
            return false;
        }
    }
    expected.owned_places = owned
        .into_iter()
        .map(|(place, multiplicity)| OwnershipFrontierOwnedPlace {
            place,
            multiplicity,
        })
        .collect();
    expected == *exit
}
