//! Borrowed edges capture a referent while their source descriptor is live.
//! A copied pointer outlives its former container; keeping a recipe through
//! that container would make later projections reload discarded storage.
//! Captures join in the existing edge fixed point. They describe addresses,
//! not new permission, and inner descriptor reads still require live storage.

use super::*;
use optimization_unit_semantics::StructuralStorageProjection;

pub(in crate::lowering) type BorrowedSources =
    std::sync::Arc<BTreeMap<PlaceId, Option<Vec<BorrowedSource>>>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::lowering) struct BorrowedSource {
    pub(super) root: ReferenceRoot,
    pub(super) projection: StructuralStorageProjection,
    pub(super) access: StructuralAccess,
    pub(super) placed: Option<target_operations::TargetPlacedInterpretation>,
}

pub(in crate::lowering) fn borrowed_source_catalog(
    functions: &BTreeMap<semantic_vocabulary::MachineId, &AbstractFunction>,
    types: &StructuralTypeLookup<'_>,
) -> BTreeMap<semantic_vocabulary::MachineId, BorrowedSources> {
    functions
        .values()
        .filter_map(|function| {
            let sources: BTreeMap<_, _> = function
                .block_structural_parameters()
                .filter(|parameter| {
                    parameter.access != StructuralAccess::Owned
                        && !terminal_semantics::descriptor_results::is_descriptor(
                            types.values().copied(),
                            parameter.structural_type,
                        )
                })
                .map(|parameter| (parameter.place, None))
                .collect();
            (!sources.is_empty()).then(|| (function.machine, std::sync::Arc::new(sources)))
        })
        .collect()
}

/// Evaluate every borrowed argument before simultaneous owner moves or cleanup.
pub(in super::super) fn capture(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    bindings: &[abstract_operations::AbstractStructuralBinding],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
) -> Result<BorrowedSources, LoweringError> {
    let mut sources = live.borrowed_reference_sources.as_ref().clone();
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if !sources.contains_key(&parameter.place) {
            continue;
        }
        let projection = storage_projection(function, live, types, &binding.argument)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if binding.parameter != parameter.place
            || binding.argument.access != parameter.access
            || !projection.matches(
                types.values().copied(),
                StructuralStorageProjection::Endpoint(parameter.structural_type),
            )
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let root = normalized_source(function, live, types, &binding.argument)?;
        // Moving a pointer's descriptor does not move its external referent.
        // Moving actual owning storage does: a simultaneous borrowed arrival
        // cannot retain the old address of that transferred subtree.
        for moved in bindings
            .iter()
            .filter(|binding| binding.argument.access == StructuralAccess::Owned)
        {
            for origin in root.origins() {
                if !matches!(origin.base,
                    target_operations::TargetReferenceOriginBase::Storage { root }
                    if root == moved.argument.place)
                {
                    continue;
                }
                let (_, path) = copies::storage_cursor(
                    function,
                    types,
                    live,
                    origin,
                    parameter.structural_type,
                )?;
                if path
                    .strip_prefix(moved.argument.path.as_slice())
                    .is_some_and(|relative| !relative.contains(&StructuralPathSegment::Referent))
                {
                    return Err(LoweringError::StructuralStateBindingMismatch {
                        machine: function.machine,
                        reason: format!(
                            "borrowed arrival {:?} retains owning storage moved by {:?}",
                            parameter.place, moved.argument
                        ),
                    });
                }
            }
        }
        sources.insert(
            parameter.place,
            Some(vec![BorrowedSource {
                root,
                projection: StructuralStorageProjection::Endpoint(parameter.structural_type),
                access: parameter.access,
                placed: placed_source(function, live, types, &binding.argument),
            }]),
        );
    }
    Ok(std::sync::Arc::new(sources))
}

pub(in super::super) fn merge(left: &mut BorrowedSources, right: &BorrowedSources) {
    for (place, sources) in std::sync::Arc::make_mut(left) {
        let Some(incoming) = right.get(place).and_then(Option::as_ref) else {
            *sources = None;
            continue;
        };
        let Some(existing) = sources.as_mut() else {
            continue;
        };
        for source in incoming {
            if !existing.contains(source) {
                existing.push(source.clone());
            }
        }
        existing.sort_by(|left, right| (&left.root, left.access).cmp(&(&right.root, right.access)));
    }
}

/// Reading a descriptor inside a captured referent needs its actual resident
/// slot. An interpretation of external bytes cannot manufacture such a slot.
pub(super) fn storage_sources(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &BorrowedSource,
) -> Result<Vec<StructuralArgument>, LoweringError> {
    let StructuralStorageProjection::Endpoint(referent) = source.projection else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    source
        .root
        .origins()
        .into_iter()
        .map(|origin| {
            let (place, path) = copies::storage_cursor(function, types, live, origin, referent)?;
            Ok(StructuralArgument {
                place,
                path,
                access: source.access,
            })
        })
        .collect()
}
