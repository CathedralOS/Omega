//! Reconcile replayed histories at one resident descriptor slot. Conditional
//! stores change its loan occurrence, not its declared storage or realization.
//! Every predecessor must supply the slot independently. Select its current
//! occurrence before joining ancestry; original formations and retired
//! generations remain distinct, just as on an owned parameter binding.

use super::occurrences::{remap_parents, resident_type};
use super::{
    BlockId, CarrierContract, Custody, LegalizationError, OccurrenceIdentity, PlaceId,
    PsiOptimizationFunction, StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, borrowed_arrivals, find_structural_type, invalid, leaf_type,
    merge_arrivals, retirements,
};
use std::collections::BTreeSet;

type Location = (PlaceId, Vec<StructuralPathSegment>);

pub(super) fn merge(
    function: &PsiOptimizationFunction,
    target: BlockId,
    types: &[StructuralTypeDeclaration],
    arrivals: impl Iterator<Item = Custody>,
) -> Result<Option<Custody>, LegalizationError> {
    let mut arrivals = arrivals.collect::<Vec<_>>();
    let first = arrivals.first().ok_or_else(invalid)?;
    let is_parameter = |place| {
        function.block(target).is_some_and(|block| {
            block.structural_parameters.iter().any(|parameter| {
                parameter.place == place && parameter.access == StructuralAccess::Owned
            })
        })
    };
    let common = first
        .leaves
        .keys()
        .filter(|location| {
            !is_parameter(location.0)
                && arrivals
                    .iter()
                    .all(|arrival| arrival.leaves.contains_key(*location))
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut selected = common
        .iter()
        .filter(|location| {
            arrivals
                .iter()
                .skip(1)
                .any(|arrival| arrival.leaves.get(*location) != first.leaves.get(*location))
        })
        .cloned()
        .collect::<BTreeSet<Location>>();
    // A surviving child also selects its ancestry when its live parent selects
    // an occurrence. A parent retired on another branch is deliberately NOT
    // rewritten on that arrival; the child retains that older generation too.
    loop {
        let identities = arrivals
            .iter()
            .flat_map(|arrival| {
                selected.iter().filter_map(|location| {
                    arrival
                        .leaves
                        .get(location)
                        .map(|leaf| leaf.identity.clone())
                })
            })
            .collect::<BTreeSet<_>>();
        let descendants = common
            .iter()
            .filter(|location| {
                !selected.contains(*location)
                    && arrivals.iter().any(|arrival| {
                        arrival.leaves.get(*location).is_some_and(|leaf| {
                            leaf.obligations().any(|parent| identities.contains(parent))
                        })
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        if descendants.is_empty() {
            break;
        }
        selected.extend(descendants);
    }
    for arrival in &mut arrivals {
        let mut identities = Vec::new();
        for location in &selected {
            let structural_type = resident_type(function, location.0).ok_or_else(invalid)?;
            let declaration = leaf_type(types, structural_type, &location.1)
                .and_then(|reference| find_structural_type(types, reference))
                .ok_or_else(invalid)?;
            let StructuralTypeShape::Reference { referent, access } = declaration.shape else {
                return Err(invalid());
            };
            let leaf = arrival.leaves.get_mut(location).ok_or_else(invalid)?;
            let carried = leaf_type(types, leaf.contract.structural_type(), &leaf.identity.path)
                .and_then(|reference| find_structural_type(types, reference))
                .ok_or_else(invalid)?;
            if carried.shape != declaration.shape
                || leaf.referent != referent
                || !matches!(
                    access,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
                || !leaf
                    .contract
                    .matches_declaration(function, leaf.identity.place)
                || leaf.formations.is_empty()
                || leaf.formations.iter().any(|formation| {
                    !formation
                        .contract
                        .matches_declaration(function, formation.identity.place)
                })
            {
                return Err(invalid());
            }
            let identity = OccurrenceIdentity::resident_join(target, location);
            // A loop with a surviving old join generation needs distinct
            // generations. Never conflate that retired ancestor with a fresh
            // live occurrence merely because its physical slot is the same.
            if arrival.retired.contains_key(&identity) {
                return Err(invalid());
            }
            identities.push((leaf.identity.clone(), identity.clone()));
            leaf.identity = identity;
            leaf.contract = CarrierContract::ResidentJoin {
                block: target,
                place: location.0,
                structural_type,
            };
        }
        remap_parents(arrival, &identities);
        arrival.validate_occurrences()?;
    }
    let mut arrivals = arrivals.into_iter();
    let mut complete = arrivals.next().ok_or_else(invalid)?;
    for arrival in arrivals {
        if merge_arrivals(
            function,
            target,
            types,
            &mut complete.leaves,
            &arrival.leaves,
        )?
        .is_none()
            || retirements::merge(&mut complete.retired, &arrival.retired).is_none()
        {
            return Ok(None);
        }
        borrowed_arrivals::merge(&mut complete, &arrival)?;
    }
    complete.prune_retired();
    complete.validate_occurrences()?;
    Ok(Some(complete))
}
