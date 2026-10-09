//! Copy descriptor storage without moving or reborrowing its permission.
//! Each destination leaf is a fresh disposal occurrence with the supplying
//! leaf's complete backing and immediate parents. The source remains live;
//! neither occurrence lends the other one's descriptor home.

use super::*;

pub(in super::super) fn copied_leaves(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    result: &StructuralOperationResult,
    source: PlaceId,
    path: &abstract_operations::StructuralCopyPath,
    operation: OperationId,
) -> Result<BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceCustody>, LoweringError> {
    let mut copied = BTreeMap::new();
    if !contains_reference(types, result.structural_type) {
        return Ok(copied);
    }
    let abstract_operations::StructuralCopyPath::Ordinary(path) = path else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let source_type = source_contract(function, live, source)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if leaf_type(types, source_type, path) != Some(result.structural_type)
        || live
            .references
            .keys()
            .any(|(carrier, _)| *carrier == result.place)
        || live.structural_homes.contains_key(&result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let paths = leaf_paths(types, result.structural_type, live.references.len())
        .filter(|paths| !paths.is_empty())
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    for relative in &paths {
        let selected_type = leaf_type(types, result.structural_type, relative)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if !types.get(&selected_type).is_some_and(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::Reference {
                    access: StructuralAccess::SharedBorrow,
                    ..
                }
            )
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut selected = path.clone();
        selected.extend_from_slice(relative);
        let mut observation = selected.clone();
        observation.push(StructuralPathSegment::Referent);
        // The same live shared observation that can read this descriptor must
        // authenticate its storage and permission before either is copied.
        // This check publishes no child and cannot skip an enclosing loan.
        access_paths::call_reference_custody(
            function,
            live,
            types,
            &StructuralArgument {
                place: source,
                path: observation,
                access: StructuralAccess::SharedBorrow,
            },
        )?;
        let original = live
            .references
            .get(&(source, selected))
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        // A copied permission retains its actual immediate parents, not the
        // source descriptor as a new parent. Each retained parent must still
        // have a live occurrence; a matching source type cannot resurrect it.
        if leaf_referent(types, result.structural_type, relative) != Some(original.referent)
            || original.parents.iter().any(|parent| {
                !live
                    .references
                    .values()
                    .any(|leaf| &leaf.identity == parent)
            })
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut leaf = original.clone();
        leaf.identity = (result.place, relative.clone());
        leaf.reform(operation, result);
        copied.insert((result.place, relative.clone()), leaf);
    }
    if live.references.keys().any(|(carrier, selected)| {
        *carrier == source
            && selected
                .strip_prefix(path.as_slice())
                .is_some_and(|relative| !paths.iter().any(|expected| expected == relative))
    }) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(copied)
}
