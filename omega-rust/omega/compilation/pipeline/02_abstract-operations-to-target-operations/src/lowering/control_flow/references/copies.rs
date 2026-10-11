//! Copy descriptor metadata with the selected storage. Psi owns copyability;
//! native lowering follows the resident paths instead of reconstructing a
//! type-wide loan roster, which would include inactive case payloads.

use super::*;

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
    // Follow descriptor associations to the actual selected storage before
    // rebasing resident leaves. The logical path can cross several carriers;
    // no leaf lives at a synthetic `root/Referent/...` metadata key.
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
                    if source_contract(function, live, origin.root)
                        .and_then(|root| leaf_type(types, root, &origin.path))
                        != Some(leaf.referent)
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    let cursor = (origin.root, origin.path);
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
    let mut candidates: BTreeMap<Vec<StructuralPathSegment>, Vec<ReferenceStorage>> =
        BTreeMap::new();
    for (carrier, mut prefix) in cursors {
        prefix.extend_from_slice(&path[segment_start..]);
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
