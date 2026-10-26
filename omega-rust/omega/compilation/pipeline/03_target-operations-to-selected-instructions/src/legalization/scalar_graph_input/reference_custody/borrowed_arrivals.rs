//! Capture borrowed storage before an edge disposes its address carrier.
//!
//! A borrowed parameter receives the original address, not a recipe for
//! rereading a dead descriptor. Its source retains normalized storage and the
//! exact immediate occurrence obligations. Retired occurrences supply ancestry
//! only; any later inner descriptor read still rejoins current resident storage.
use super::*;
use abstract_operations::AbstractStructuralBinding;
use optimization_unit_semantics::StructuralStorageProjection;
use terminal_psi::StructuralParameterDeclaration;

// Fixed-point propagation retains unknown predecessors explicitly. Consuming
// an unknown arrival refuses; it never becomes an activation-local owning root.
pub(super) type Sources = BTreeMap<PlaceId, Option<Vec<Source>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Source {
    root: Root,
    projection: StructuralStorageProjection,
    access: StructuralAccess,
    pub(super) parents: Vec<OccurrenceIdentity>,
}

fn canonicalize(sources: &mut Vec<Source>) {
    sources.sort_by(|left, right| {
        (&left.root, &left.parents, left.access).cmp(&(&right.root, &right.parents, right.access))
    });
    sources.dedup();
}

/// Resolve current typed storage only when a consumer actually needs to read
/// an inner descriptor. Captured byte geometry cannot manufacture such a slot.
pub(super) fn flatten(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<Vec<StructuralArgument>> {
    let Some(origins) = custody.borrowed_sources.get(&source.place) else {
        return Some(vec![source.clone()]);
    };
    let mut resolved = Vec::new();
    for origin in origins.as_ref()? {
        if !source.access.admits(origin.access) {
            return None;
        }
        let StructuralStorageProjection::Endpoint(referent) = origin.projection else {
            return None;
        };
        for storage in storage_origins(function, custody, types, &origin.root, referent).ok()? {
            let mut path = storage.exact_path()?;
            path.extend_from_slice(&source.path);
            resolved.push(StructuralArgument {
                place: storage.base.root_place()?,
                path,
                access: source.access,
            });
        }
    }
    (!resolved.is_empty()).then_some(resolved)
}

pub(super) fn normalize(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Result<(Root, Vec<OccurrenceIdentity>), LegalizationError> {
    let Some(origins) = custody.borrowed_sources.get(&source.place) else {
        return normalized_direct_source(function, custody, types, source);
    };
    let origins = origins
        .as_ref()
        .filter(|origins| !origins.is_empty())
        .ok_or_else(|| invalid())?;
    if source.path.contains(&StructuralPathSegment::Referent) {
        return combine_candidate_origins(
            flatten(function, custody, types, source)
                .ok_or_else(|| invalid())?
                .iter()
                .map(|actual| normalized_direct_source(function, custody, types, actual))
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    // Presentation is checked at the receiving coordinate. It is not a demand
    // to reinterpret the original byte storage as a newly declared value.
    storage_projection(function, custody, types, source).ok_or_else(|| invalid())?;
    let mut candidates = Vec::with_capacity(origins.len());
    for origin in origins {
        if !source.access.admits(origin.access) {
            return Err(invalid());
        }
        candidates.push((origin.root.projected(&source.path), origin.parents.clone()));
    }
    combine_candidate_origins(candidates)
}

/// Capture every actual while all source descriptors still exist. Owned
/// transfer has already selected the receiving leaf occurrences; remap only
/// those exact identities, never equal backing or a declared reference type.
pub(super) fn bind(
    function: &PsiOptimizationFunction,
    before: &Custody,
    after: &mut Custody,
    bindings: &[AbstractStructuralBinding],
    parameters: &[StructuralParameterDeclaration],
    types: &[StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
    if bindings.len() != parameters.len() {
        return Err(invalid());
    }
    let mut identities = Vec::new();
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if binding.argument.access != StructuralAccess::Owned
            || parameter.access != StructuralAccess::Owned
        {
            continue;
        }
        for (location, leaf) in &before.leaves {
            if location.0 == binding.argument.place
                && let Some(relative) = owned_relative_path(&location.1, &binding.argument.path)
            {
                let destination = (parameter.place, relative.to_vec());
                let arrived = after.leaves.get(&destination).ok_or_else(|| invalid())?;
                if arrived.identity != OccurrenceIdentity::parameter(&destination)
                    || arrived.contract != CarrierContract::Parameter(parameter.clone())
                {
                    return Err(invalid());
                }
                identities.push((leaf.identity.clone(), arrived.identity.clone()));
            }
        }
    }
    let mut sources = before.borrowed_sources.as_ref().clone();
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if !sources.contains_key(&parameter.place) {
            continue;
        }
        if !parameter.access.admits(binding.argument.access) {
            return Err(invalid());
        }
        let captured = (|| {
            let projection = storage_projection(function, before, types, &binding.argument)?;
            if !projection.matches(
                types.iter(),
                StructuralStorageProjection::Endpoint(parameter.structural_type),
            ) {
                return None;
            }
            let (root, parents) = normalize(function, before, types, &binding.argument).ok()?;
            // Moving an owning home cannot leave an address to its former
            // storage behind. Only descriptor occurrence transport is rebased.
            for binding in bindings
                .iter()
                .filter(|binding| binding.argument.access == StructuralAccess::Owned)
            {
                for origin in root.origins() {
                    // Captured incoming contents are the pointee value, not
                    // the descriptor slot named in their provenance. Moving
                    // that slot cannot move the external pointee.
                    if !matches!(origin.base, target_operations::TargetReferenceOriginBase::Storage { root } if root == binding.argument.place)
                    {
                        continue;
                    }
                    let cursors = storage_origins(
                        function,
                        before,
                        types,
                        &Root::Alternatives(vec![origin]),
                        parameter.structural_type,
                    )
                    .ok()?;
                    for cursor in cursors {
                        let path = cursor.exact_path()?;
                        if owned_relative_path(&path, &binding.argument.path).is_some() {
                            return None;
                        }
                    }
                }
            }
            Some(vec![Source {
                root,
                projection: StructuralStorageProjection::Endpoint(parameter.structural_type),
                access: parameter.access,
                parents,
            }])
        })();
        sources.insert(parameter.place, captured);
    }
    after.borrowed_sources = std::sync::Arc::new(sources);
    remap_parents(after, &identities);
    Ok(())
}

/// A borrowed arrival carries authority from its exact captured occurrences,
/// not from equal storage geometry. Every possible incoming source must grant
/// this access through the same parent; missing predecessors stay unknown.
/// Captured authority also traverses that parent's exact ancestry. Descendants
/// and separate loans still interfere; lifetime debts remain independent.
pub(super) fn lends_from(
    custody: &Custody,
    place: PlaceId,
    parent: &OccurrenceIdentity,
    requested: StructuralAccess,
) -> bool {
    custody
        .borrowed_sources
        .get(&place)
        .and_then(Option::as_ref)
        .is_some_and(|sources| {
            !sources.is_empty()
                && sources.iter().all(|source| {
                    if !requested.admits(source.access) {
                        return false;
                    }
                    let mut pending = source.parents.iter().collect::<Vec<_>>();
                    let mut visited = BTreeSet::new();
                    while let Some(ancestor) = pending.pop() {
                        if ancestor == parent {
                            return true;
                        }
                        if !visited.insert(ancestor) {
                            continue;
                        }
                        for leaf in custody
                            .leaves
                            .values()
                            .chain(custody.retired.values().map(|retired| &retired.leaf))
                        {
                            if &leaf.identity == ancestor {
                                pending.extend(&leaf.parents);
                            }
                        }
                    }
                    false
                })
        })
}

pub(super) fn obligations(custody: &Custody) -> impl Iterator<Item = &OccurrenceIdentity> {
    custody
        .borrowed_sources
        .values()
        .flatten()
        .flatten()
        .flat_map(|source| &source.parents)
}

pub(super) fn remap_parents(
    custody: &mut Custody,
    identities: &[(OccurrenceIdentity, OccurrenceIdentity)],
) {
    if identities.is_empty() {
        return;
    }
    for sources in std::sync::Arc::make_mut(&mut custody.borrowed_sources)
        .values_mut()
        .flatten()
    {
        for source in sources.iter_mut() {
            source.parents = source
                .parents
                .iter()
                .flat_map(|parent| {
                    let moved = identities
                        .iter()
                        .filter(|(original, _)| original == parent)
                        .map(|(_, destination)| destination.clone())
                        .collect::<Vec<_>>();
                    if moved.is_empty() {
                        vec![parent.clone()]
                    } else {
                        moved
                    }
                })
                .collect();
            source.parents.sort();
            source.parents.dedup();
        }
        canonicalize(sources);
    }
}

/// Every arrival contributes; an unknown predecessor never donates evidence.
pub(super) fn merge(left: &mut Custody, right: &Custody) -> Result<(), LegalizationError> {
    if !left
        .borrowed_sources
        .keys()
        .eq(right.borrowed_sources.keys())
    {
        return Err(invalid());
    }
    let sources = std::sync::Arc::make_mut(&mut left.borrowed_sources);
    for (place, incoming) in right.borrowed_sources.iter() {
        let existing = sources.get_mut(place).ok_or_else(|| invalid())?;
        if let (Some(existing), Some(incoming)) = (existing.as_mut(), incoming.as_ref()) {
            existing.extend_from_slice(incoming);
            canonicalize(existing);
        } else {
            *existing = None;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occurrence(number: u64) -> OccurrenceIdentity {
        OccurrenceIdentity::parameter(&(PlaceId::new(number).unwrap(), Vec::new()))
    }

    fn source(parent: OccurrenceIdentity, access: StructuralAccess) -> Source {
        Source {
            root: Root::Ingress(PlaceId::new(1).unwrap(), Vec::new()),
            projection: StructuralStorageProjection::Endpoint(StructuralTypeId::new(1).unwrap()),
            access,
            parents: vec![parent],
        }
    }

    #[test]
    fn borrowed_arrival_requires_exact_parent_not_equal_backing() {
        let place = PlaceId::new(2).unwrap();
        let parent = occurrence(3);
        let mut custody = Custody::default();
        std::sync::Arc::make_mut(&mut custody.borrowed_sources).insert(
            place,
            Some(vec![source(
                parent.clone(),
                StructuralAccess::MutableBorrow,
            )]),
        );
        assert!(lends_from(
            &custody,
            place,
            &parent,
            StructuralAccess::MutableBorrow
        ));
        assert!(!lends_from(
            &custody,
            place,
            &occurrence(4),
            StructuralAccess::MutableBorrow
        ));
        let replaced_parent = OccurrenceIdentity::operation(
            &(PlaceId::new(3).unwrap(), Vec::new()),
            OperationId::new(7).unwrap(),
        );
        assert!(!lends_from(
            &custody,
            place,
            &replaced_parent,
            StructuralAccess::MutableBorrow
        ));
        assert!(!lends_from(
            &custody,
            place,
            &parent,
            StructuralAccess::Owned
        ));
        // Equal backing on a second incoming edge cannot supply the first
        // edge's captured permission occurrence.
        std::sync::Arc::make_mut(&mut custody.borrowed_sources)
            .get_mut(&place)
            .unwrap()
            .as_mut()
            .unwrap()
            .push(source(occurrence(4), StructuralAccess::MutableBorrow));
        assert!(!lends_from(
            &custody,
            place,
            &parent,
            StructuralAccess::MutableBorrow
        ));
    }

    #[test]
    fn borrowed_arrival_preserves_access_and_unknown_predecessors() {
        let place = PlaceId::new(2).unwrap();
        let parent = occurrence(3);
        let mut custody = Custody::default();
        for sources in [None, Some(Vec::new())] {
            std::sync::Arc::make_mut(&mut custody.borrowed_sources).insert(place, sources);
            assert!(!lends_from(
                &custody,
                place,
                &parent,
                StructuralAccess::SharedBorrow
            ));
        }
        std::sync::Arc::make_mut(&mut custody.borrowed_sources).insert(
            place,
            Some(vec![source(parent.clone(), StructuralAccess::SharedBorrow)]),
        );
        assert!(lends_from(
            &custody,
            place,
            &parent,
            StructuralAccess::SharedBorrow
        ));
        assert!(!lends_from(
            &custody,
            place,
            &parent,
            StructuralAccess::MutableBorrow
        ));
        assert!(!lends_from(
            &custody,
            place,
            &parent,
            StructuralAccess::WriteOnlyBorrow
        ));
    }
}
