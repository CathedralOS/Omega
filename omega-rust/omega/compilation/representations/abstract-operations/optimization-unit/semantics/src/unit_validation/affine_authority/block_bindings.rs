//! Rejoin retained edge snapshots through the edge's owned rebinding and cleanup.
//!
//! The transition follows the current-ownership replay's edge transaction:
//! owned sources leave the entry frontier first (a projected move opens or
//! extends partial custody on its root), the edge's cleanup stream then
//! retires each root it names, and the target's roots and transferred claims
//! are installed last. The replay of the same unit authenticates every cleanup
//! action against the exact live subject partition, eligibility order and
//! claim roster; this check conserves the retained snapshots' net custody
//! change across the current edge.
use abstract_operations::AbstractStructuralBinding;
use optimization_unit::{
    OwnershipFrontierLiveClaim, OwnershipFrontierOwnedPlace, OwnershipFrontierPartialCustody,
    OwnershipFrontierSnapshot, PsiOptimizationFunction,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, TerminalAffineCleanupAction,
};

pub(super) fn valid_transition(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    entry: &OwnershipFrontierSnapshot,
    exit: &OwnershipFrontierSnapshot,
    actions: &[TerminalAffineCleanupAction],
    bindings: &[AbstractStructuralBinding],
) -> bool {
    let Some(consumed) = consume_sources(function, types, entry, bindings) else {
        return false;
    };
    let Some(mut expected) = retire_cleanup_roots(function, types, consumed.frontier, actions)
    else {
        return false;
    };
    let mut owned = expected
        .owned_places
        .iter()
        .map(|owned| (owned.place, owned.multiplicity))
        .collect::<BTreeMap<_, _>>();
    for parameter in consumed.arrivals {
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
    for claim in consumed.claims {
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

/// Each root's ordered disposal partition ends in explicit frame retirement,
/// except intrinsic whole-owner disposal. Hooks consume only named claims.
fn retire_cleanup_roots(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    mut frontier: OwnershipFrontierSnapshot,
    actions: &[TerminalAffineCleanupAction],
) -> Option<OwnershipFrontierSnapshot> {
    let mut retired = Vec::<PlaceId>::new();
    let mut remaining = actions;
    while let Some(action) = remaining.first() {
        let place = action.place();
        if retired.contains(&place) {
            return None;
        }
        let count = remaining
            .iter()
            .take_while(|action| action.place() == place)
            .count();
        let (group, rest) = remaining.split_at(count);
        let partition = match group.split_last()? {
            (TerminalAffineCleanupAction::RetireHome(retired), partition) if *retired == place => {
                partition
            }
            _ if matches!(group, [TerminalAffineCleanupAction::DiscardRoot(_)])
                || matches!(group, [TerminalAffineCleanupAction::InvokeNominal(cleanup)] if cleanup.path.is_empty()) =>
            {
                group
            }
            _ => return None,
        };
        for action in partition {
            if matches!(action, TerminalAffineCleanupAction::RetireHome(_)) {
                return None;
            }
            if let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action {
                for consumed in &cleanup.consumed_claims {
                    let position = frontier
                        .claims
                        .iter()
                        .position(|claim| claim.claim == *consumed && claim.input == Some(place))?;
                    frontier.claims.remove(position);
                }
            }
        }
        if frontier
            .claims
            .iter()
            .any(|claim| claim.input == Some(place))
        {
            return None;
        }
        if let Some(position) = frontier
            .owned_places
            .iter()
            .position(|owned| owned.place == place)
        {
            frontier.owned_places.remove(position);
        } else {
            let custody = frontier
                .partial_custody
                .iter()
                .find(|custody| custody.place == place)?;
            let moved = custody.moved_paths.iter().cloned().collect::<BTreeSet<_>>();
            if !matches!(group, [TerminalAffineCleanupAction::RetireHome(_)])
                || !crate::current_ownership::projected_paths_are_fully_consumed(
                    function, types, place, &moved,
                )
            {
                return None;
            }
        }
        frontier
            .partial_custody
            .retain(|custody| custody.place != place);
        retired.push(place);
        remaining = rest;
    }
    Some(frontier)
}

/// The entry frontier after every owned source has left it, with the
/// destination roots and renamed claims still waiting to be installed.
struct ConsumedSources<'function> {
    frontier: OwnershipFrontierSnapshot,
    arrivals: Vec<&'function StructuralParameterDeclaration>,
    claims: Vec<OwnershipFrontierLiveClaim>,
}

/// Signature replay checks each exact source/destination type, access and
/// multiplicity. Read claim custody from the immutable entry snapshot, then
/// remove every source before any arrival exists, preserving swaps and
/// self-loops without treating a new destination as another old source.
fn consume_sources<'function>(
    function: &'function PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    entry: &OwnershipFrontierSnapshot,
    bindings: &[AbstractStructuralBinding],
) -> Option<ConsumedSources<'function>> {
    let mut frontier = entry.clone();
    let mut arrivals = Vec::new();
    let mut claims = Vec::new();
    let mut removed_claims = BTreeSet::new();
    for binding in bindings {
        let parameter = function
            .block_structural_parameters()
            .find(|parameter| parameter.place == binding.parameter)?;
        if parameter.access != StructuralAccess::Owned {
            continue;
        }
        let source = binding.argument.place;
        if binding.argument.access != StructuralAccess::Owned {
            return None;
        }
        let source_claims = entry
            .claims
            .iter()
            .filter(|claim| claim.input == Some(source))
            .collect::<Vec<_>>();
        if !binding.argument.path.is_empty() {
            if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
                let source_contract =
                    super::super::operation_contracts::structural_source_contract(
                        function, source,
                    )?;
                if source_contract.access != StructuralAccess::Owned
                    || source_contract.multiplicity != StructuralMultiplicity::Unrestricted
                    || !source_claims.is_empty()
                    || frontier
                        .owned_places
                        .iter()
                        .any(|owned| owned.place == source)
                    || frontier
                        .partial_custody
                        .iter()
                        .any(|custody| custody.place == source)
                {
                    return None;
                }
                // Copying has no net ownership-frontier change. The signature
                // checks selected type/memberships, and current replay checks
                // reads against live loans before accepting this snapshot.
                continue;
            }
            // A projected move takes one affine child of a claim-free affine
            // root; the root keeps the complement under partial custody.
            if parameter.multiplicity != StructuralMultiplicity::Affine
                || !source_claims.is_empty()
                || !move_projected_child(function, types, &mut frontier, source, binding)
            {
                return None;
            }
            arrivals.push(parameter);
            continue;
        }
        if frontier
            .partial_custody
            .iter()
            .any(|custody| custody.place == source)
            || (parameter.multiplicity == StructuralMultiplicity::Linear
                && source_claims.is_empty())
            || (parameter.multiplicity != StructuralMultiplicity::Linear
                && !source_claims.is_empty())
        {
            return None;
        }
        if parameter.multiplicity != StructuralMultiplicity::Unrestricted {
            let position = frontier.owned_places.iter().position(|owned| {
                owned.place == source && owned.multiplicity == parameter.multiplicity
            })?;
            frontier.owned_places.remove(position);
            arrivals.push(parameter);
        }
        for claim in source_claims {
            if !removed_claims.insert(claim.claim) {
                return None;
            }
            let mut moved = claim.clone();
            moved.input = Some(parameter.place);
            claims.push(moved);
        }
    }
    frontier
        .claims
        .retain(|claim| !removed_claims.contains(&claim.claim));
    Some(ConsumedSources {
        frontier,
        arrivals,
        claims,
    })
}

/// Record one projected owned move on its root's partial custody row. A root
/// whose affine complement is now empty leaves the owned set at this edge, as
/// it does in the current-ownership replay.
fn move_projected_child(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    frontier: &mut OwnershipFrontierSnapshot,
    source: PlaceId,
    binding: &AbstractStructuralBinding,
) -> bool {
    let path = &binding.argument.path;
    if !frontier
        .owned_places
        .iter()
        .any(|owned| owned.place == source && owned.multiplicity == StructuralMultiplicity::Affine)
    {
        return false;
    }
    let position = match frontier
        .partial_custody
        .binary_search_by_key(&source, |custody| custody.place)
    {
        Ok(position) => position,
        Err(position) => {
            frontier.partial_custody.insert(
                position,
                OwnershipFrontierPartialCustody {
                    place: source,
                    moved_paths: Vec::new(),
                },
            );
            position
        }
    };
    let moved = &mut frontier.partial_custody[position].moved_paths;
    if moved
        .iter()
        .any(|existing| existing.starts_with(path) || path.starts_with(existing))
    {
        return false;
    }
    let insertion = moved
        .binary_search(path)
        .unwrap_or_else(|position| position);
    moved.insert(insertion, path.clone());
    let moved = moved.iter().cloned().collect::<BTreeSet<_>>();
    if crate::current_ownership::partial_affine_root_is_drained(function, types, source, &moved) {
        frontier.owned_places.retain(|owned| owned.place != source);
    }
    true
}
