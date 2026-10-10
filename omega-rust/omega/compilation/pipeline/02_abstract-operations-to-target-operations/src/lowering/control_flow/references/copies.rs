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
    for ((carrier, selected), original) in &live.references {
        if *carrier != source {
            continue;
        }
        let Some(relative) = selected.strip_prefix(path.as_ref()) else {
            continue;
        };
        let carrier_type = leaf_type(types, result.structural_type, relative)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if carrier_type != original.carrier_type
            || leaf_referent(types, result.structural_type, relative) != Some(original.referent)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        copied.insert(
            (result.place, relative.to_vec()),
            ReferenceStorage {
                root: original.root.clone(),
                carrier_type,
                referent: original.referent,
            },
        );
    }
    Ok(copied)
}
