//! Copy descriptor metadata with the selected storage. Psi owns copyability;
//! native lowering follows the resident paths instead of reconstructing a
//! type-wide loan roster, which would include inactive case payloads.

use super::*;

/// Resolve the actual storage selected by a logical path through current descriptors.
/// Copies may join alternatives; an exact storage mutation must select one region.
pub(in super::super) fn storage_regions(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    source: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<Vec<(PlaceId, Vec<StructuralPathSegment>)>, LoweringError> {
    // Follow descriptor associations to the actual selected storage before
    // rebasing resident leaves. The logical path can cross several carriers;
    // incoming descriptor origins acquire a typed Referent storage cursor;
    // explicit writes may install known leaves there without seeding pointees.
    let mut cursors = vec![(source, Vec::new())];
    let mut segment_start = 0;
    let budget = live
        .references
        .len()
        .max(1)
        .checked_mul(path.len().max(1))
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    for (position, segment) in path.iter().enumerate() {
        if *segment != StructuralPathSegment::Referent {
            continue;
        }
        let mut next = Vec::new();
        for (carrier, mut prefix) in cursors {
            prefix.extend_from_slice(&path[segment_start..position]);
            let root = source_contract(function, live, carrier)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            let selected = terminal_semantics::stored_descriptor_paths(
                types.values().copied(),
                root,
                &prefix,
                budget,
            )
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            for selected in selected {
                let leaf = live
                    .references
                    .get(&(carrier, selected.clone()))
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                if leaf_type(types, root, &selected) != Some(leaf.carrier_type) {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                for origin in leaf.root.origins() {
                    let cursor = storage_cursor(function, types, live, origin, leaf.referent)?;
                    if !next.contains(&cursor) {
                        if next.len() >= budget {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        }
                        next.push(cursor);
                    }
                }
            }
        }
        cursors = next;
        segment_start = position + 1;
    }
    for (_, prefix) in &mut cursors {
        prefix.extend_from_slice(&path[segment_start..]);
    }
    Ok(cursors)
}

/// Ingress custody names the incoming descriptor, while a storage cursor
/// names its pointee. Only an exact descriptor in a declared parameter's owned
/// payload supplies that implicit crossing; retained target origins stay intact.
fn storage_cursor(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    origin: target_operations::TargetReferenceOrigin,
    expected: StructuralTypeId,
) -> Result<(PlaceId, Vec<StructuralPathSegment>), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let mut path = origin.exact_path().ok_or_else(invalid)?;
    let place = origin.base.root_place().ok_or_else(invalid)?;
    let root = source_contract(function, live, place).ok_or_else(invalid)?;
    let selected = leaf_type(types, root, &path).ok_or_else(invalid)?;
    if selected != expected {
        let parameter = function.structural_parameter(place).ok_or_else(invalid)?;
        if parameter.structural_type != root
            || path.iter().any(|segment| {
                !matches!(
                    segment,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::Case(_)
                        | StructuralPathSegment::FixedIndex(_)
                )
            })
            || !matches!(types.get(&selected).map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Reference { referent, .. }) if *referent == expected)
        {
            return Err(invalid());
        }
        path.push(StructuralPathSegment::Referent);
    }
    Ok((place, path))
}

pub(in super::super) fn copied_leaves(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    result: &StructuralOperationResult,
    source: PlaceId,
    path: &abstract_operations::StructuralCopyPath,
) -> Result<BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>, LoweringError> {
    let mut copied = BTreeMap::new();
    if !contains_reference(types, result.structural_type) {
        return Ok(copied);
    }
    let source_type = source_contract(function, live, source)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let path: std::borrow::Cow<'_, [StructuralPathSegment]> = match path {
        abstract_operations::StructuralCopyPath::Ordinary(path) => std::borrow::Cow::Borrowed(path),
        abstract_operations::StructuralCopyPath::Case(path) => std::borrow::Cow::Owned(
            terminal_semantics::canonical_structural_storage_path(
                types.values().copied(),
                source_type,
                path,
            )
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?,
        ),
    };
    if leaf_type(types, source_type, &path) != Some(result.structural_type)
        || live
            .references
            .keys()
            .any(|(carrier, _)| *carrier == result.place)
        || live.structural_homes.contains_key(&result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let cursors = storage_regions(function, types, live, source, &path)?;
    let mut candidates: BTreeMap<Vec<StructuralPathSegment>, Vec<ReferenceStorage>> =
        BTreeMap::new();
    for (carrier, prefix) in cursors {
        let root = source_contract(function, live, carrier)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let selected = terminal_semantics::StructuralStorageSelection::new(
            types.values().copied(),
            root,
            &prefix,
        )
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if leaf_type(types, root, &prefix) != Some(result.structural_type) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        for ((resident, path), original) in &live.references {
            if *resident != carrier {
                continue;
            }
            let Some(relative) = selected.relative_path(path) else {
                continue;
            };
            if relative.contains(&StructuralPathSegment::Referent) {
                continue;
            }
            let carrier_type = leaf_type(types, result.structural_type, relative)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            if carrier_type != original.carrier_type
                || leaf_referent(types, result.structural_type, relative) != Some(original.referent)
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            candidates
                .entry(relative.to_vec())
                .or_default()
                .push(original.clone());
        }
    }
    for (relative, alternatives) in candidates {
        copied.insert(
            (result.place, relative),
            combine_alternatives(function, alternatives)?,
        );
    }
    Ok(copied)
}
