//! Follow nonescaping accesses through their current live descriptors.
//! Every inner coordinate must rejoin concrete typed storage, while intermediate
//! descriptor dependencies remain separate from the final borrowed origins.

use super::{
    AbstractFunction, LiveDefinitions, LoweringError, ReferenceRoot, StructuralAccess,
    StructuralArgument, StructuralPathSegment, StructuralTypeId, StructuralTypeLookup,
    StructuralTypeShape, leaf_type, shared_source_compatible, source_contract,
};

/// A nonescaping access follows live descriptor custody at every crossing.
/// Intermediate origins become concrete cursors only after their current
/// storage contract matches the declared referent. Opaque ingress permissions
/// remain valid final origins, but cannot invent another stored descriptor.
pub(super) struct AccessReferenceCustody {
    pub(super) origins: Vec<ReferenceRoot>,
    pub(super) dependencies: Vec<ReferenceRoot>,
}

/// Calls keep their static structural endpoint grammar. Scalar observations
/// have their own endpoint operation and use the same custody walk below.
pub(super) fn call_reference_custody(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    argument: &StructuralArgument,
) -> Result<AccessReferenceCustody, LoweringError> {
    let mut current = source_contract(function, live, argument.place)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    for (position, segment) in argument.path.iter().enumerate() {
        current = match segment {
            StructuralPathSegment::Referent => {
                let StructuralTypeShape::Reference { referent, .. } = types
                    .get(&current)
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?
                    .shape
                else {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                };
                referent
            }
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_) => {
                leaf_type(types, current, std::slice::from_ref(segment))
                    .or_else(|| {
                        (argument.access == StructuralAccess::SharedBorrow
                            && position + 1 == argument.path.len())
                        .then(|| {
                            terminal_semantics::shared_primitive_projection_declaration(
                                types.values().copied(),
                                current,
                                std::slice::from_ref(segment),
                            )
                        })
                        .flatten()
                    })
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?
            }
            _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
        };
    }
    access_reference_custody(function, live, types, argument)
}

pub(super) fn access_reference_custody(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    argument: &StructuralArgument,
) -> Result<AccessReferenceCustody, LoweringError> {
    if argument.access == StructuralAccess::Owned {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let source_access = function
        .arrival_parameter(argument.place)
        .map(|parameter| parameter.access)
        .or_else(|| {
            live.structural_homes
                .get(&argument.place)
                .map(|home| home.access())
        })
        .unwrap_or(StructuralAccess::Owned);
    if source_access == StructuralAccess::WriteOnlyBorrow
        || source_access == StructuralAccess::SharedBorrow
            && argument.access != StructuralAccess::SharedBorrow
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut current = source_contract(function, live, argument.place)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let mut cursors = vec![(argument.place, Vec::new(), Vec::new())];
    let cursor_budget = live
        .references
        .len()
        .checked_mul(argument.path.len())
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let mut dependencies = Vec::new();
    let mut prefix_start = 0;
    for (position, segment) in argument.path.iter().enumerate() {
        if *segment != StructuralPathSegment::Referent {
            continue;
        }
        let prefix = &argument.path[prefix_start..position];
        let reference_type = terminal_semantics::runtime_structural_path_tip(
            types.values().copied(),
            current,
            prefix,
        )
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?
        .id;
        let StructuralTypeShape::Reference { referent, access } = types
            .get(&reference_type)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?
            .shape
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let further_crossing =
            argument.path[position + 1..].contains(&StructuralPathSegment::Referent);
        if !(access == argument.access
            || access == StructuralAccess::MutableBorrow
                && matches!(
                    argument.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
            || further_crossing && access == StructuralAccess::WriteOnlyBorrow
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut origins = Vec::new();
        let mut next_cursors = Vec::new();
        for (place, mut selected_path, ancestors) in cursors {
            selected_path.extend_from_slice(prefix);
            let root_type = source_contract(function, live, place)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            let candidates = descriptor_candidates(
                function,
                types,
                root_type,
                &selected_path,
                live.references.len(),
            )
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            for path in candidates {
                if source_contract(function, live, place)
                    .and_then(|contract| leaf_type(types, contract, &path))
                    != Some(reference_type)
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let leaf = live
                    .references
                    .get(&(place, path.clone()))
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                if leaf.referent != referent
                    || live.references.iter().any(|((carrier, path), child)| {
                        child.parents.contains(&leaf.identity)
                            && !shared_source_compatible(
                                function,
                                live,
                                types,
                                carrier,
                                path,
                                argument.access,
                            )
                    })
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                // A descriptor is stored data too. Direct access cannot bypass
                // a live enclosing loan; traversing that exact ancestor is the
                // only non-shared exemption, retained separately on each cursor.
                if live
                    .references
                    .iter()
                    .any(|((carrier, carrier_path), enclosing)| {
                        !ancestors.contains(&enclosing.identity)
                            && !shared_source_compatible(
                                function,
                                live,
                                types,
                                carrier,
                                carrier_path,
                                argument.access,
                            )
                            && enclosing.root.origins().iter().any(|origin| {
                                let origin_path = &origin.path;
                                let origin_place = origin.root;
                                origin_place == place
                                    && source_contract(function, live, origin_place).and_then(
                                        |contract| leaf_type(types, contract, origin_path),
                                    ) == Some(enclosing.referent)
                                    && terminal_semantics::structural_paths_may_overlap(
                                        origin_path,
                                        &path,
                                    )
                            })
                    })
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                dependencies.push(ReferenceRoot::Ingress(place, path));
                origins.push(leaf.root.clone());
                if further_crossing {
                    dependencies.push(leaf.root.clone());
                    let mut ancestors = ancestors.clone();
                    ancestors.push(leaf.identity.clone());
                    for origin in leaf.root.origins() {
                        let place = origin.root;
                        let path = origin.path;
                        if source_contract(function, live, place)
                            .and_then(|contract| leaf_type(types, contract, &path))
                            != Some(referent)
                        {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        }
                        let cursor = (place, path, ancestors.clone());
                        if !next_cursors.contains(&cursor) {
                            if next_cursors.len() >= cursor_budget {
                                return Err(LoweringError::unsupported_control_flow(
                                    function.machine,
                                ));
                            }
                            next_cursors.push(cursor);
                        }
                    }
                }
            }
        }
        if !further_crossing {
            // The access operation independently validates final endpoint
            // geometry. Custody never invents a primitive structural type for
            // an inline field or drops an earlier runtime selector.
            if origins.is_empty() {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            return Ok(AccessReferenceCustody {
                origins,
                dependencies,
            });
        }
        cursors = next_cursors;
        current = referent;
        prefix_start = position + 1;
    }
    Err(LoweringError::unsupported_control_flow(function.machine))
}

/// Enumerate exact stored descriptors from the declared fixed array extents.
/// Missing live rows cannot shrink the required coverage; the roster bounds
/// construction before any dynamic product is expanded.
fn descriptor_candidates(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    mut current: StructuralTypeId,
    path: &[StructuralPathSegment],
    maximum: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
    if maximum == 0 {
        return None;
    }
    let mut candidates = vec![Vec::new()];
    let path = abstract_operations::values::resolve_constant_runtime_indexes(
        function.operations.iter(),
        path,
    );
    for segment in &path {
        if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
            let StructuralTypeShape::FixedArray { element, length } = types.get(&current)?.shape
            else {
                return None;
            };
            let length = usize::try_from(length).ok()?;
            let count = candidates.len().checked_mul(length)?;
            if count == 0 || count > maximum {
                return None;
            }
            let mut expanded = Vec::with_capacity(count);
            for candidate in candidates {
                for element_index in 0..length {
                    let mut selected = candidate.clone();
                    selected.push(StructuralPathSegment::FixedIndex(
                        u64::try_from(element_index).ok()?,
                    ));
                    expanded.push(selected);
                }
            }
            candidates = expanded;
            current = element;
        } else {
            current = leaf_type(types, current, std::slice::from_ref(segment))?;
            for candidate in &mut candidates {
                candidate.push(segment.clone());
            }
        }
    }
    Some(candidates)
}
