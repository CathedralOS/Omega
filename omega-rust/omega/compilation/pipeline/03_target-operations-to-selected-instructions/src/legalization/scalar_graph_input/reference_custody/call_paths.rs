//! Follow borrowed observations through their current live descriptors.
//! Every inner coordinate must rejoin concrete typed storage, while intermediate
//! descriptor dependencies remain separate from the final borrowed origins.

use super::{
    Custody, LegalizationError, PsiOptimizationFunction, Root, StructuralAccess,
    StructuralArgument, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    invalid, leaf_type, shared_source_compatible, source_contract,
};

/// A nonescaping call follows live descriptor custody at every crossing.
/// Intermediate origins become concrete cursors only after their current
/// storage contract matches the declared referent. Opaque ingress permissions
/// remain valid final origins, but cannot invent another stored descriptor.
pub(super) struct ReferenceObservation {
    pub(super) origins: Vec<Root>,
    pub(super) referent: semantic_vocabulary::StructuralTypeId,
    pub(super) dependencies: Vec<Root>,
    pub(super) descriptors: Vec<(semantic_vocabulary::PlaceId, Vec<StructuralPathSegment>)>,
    pub(super) borrowed: Vec<super::observations::Observed>,
    absent_case_descriptors: bool,
}

pub(super) fn call_reference_custody(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    argument: &StructuralArgument,
) -> Result<ReferenceObservation, LegalizationError> {
    let observed = observe_reference_custody(function, custody, types, argument)?;
    if observed.absent_case_descriptors || observed.origins.is_empty() {
        return Err(invalid());
    }
    Ok(observed)
}

/// Observe actual descriptor storage without manufacturing inactive case
/// payloads. A call requires every selected descriptor; a subtree snapshot
/// copies only resident case leaves. Missing unconditional storage and invalid
/// access still fail here, before either consumer can use the observation.
pub(super) fn observe_reference_custody(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    argument: &StructuralArgument,
) -> Result<ReferenceObservation, LegalizationError> {
    if !custody.borrowed_sources.contains_key(&argument.place) {
        return observe_direct_reference_custody(function, custody, types, argument);
    }
    // A borrowed aggregate parameter transports an address, not duplicate
    // resident permissions. Authenticate its complete root presentation first,
    // then walk the unchanged relative path in every actual source namespace.
    // Looking up a descriptor at the parameter itself would lose that roster;
    // flattening the full path would demand the same missing descriptor first.
    let roots = super::borrowed_arrivals::flatten(
        function,
        custody,
        types,
        &StructuralArgument {
            place: argument.place,
            path: Vec::new(),
            access: argument.access,
        },
    )
    .ok_or_else(invalid)?;
    let mut complete: Option<ReferenceObservation> = None;
    for mut root in roots {
        root.path.extend_from_slice(&argument.path);
        root.access = argument.access;
        let observed = observe_direct_reference_custody(function, custody, types, &root)?;
        if let Some(complete) = &mut complete {
            if complete.referent != observed.referent {
                return Err(invalid());
            }
            complete.absent_case_descriptors |= observed.absent_case_descriptors;
            for origin in observed.origins {
                if !complete.origins.contains(&origin) {
                    complete.origins.push(origin);
                }
            }
            for dependency in observed.dependencies {
                if !complete.dependencies.contains(&dependency) {
                    complete.dependencies.push(dependency);
                }
            }
            for descriptor in observed.descriptors {
                if !complete.descriptors.contains(&descriptor) {
                    complete.descriptors.push(descriptor);
                }
            }
            complete.borrowed.extend(observed.borrowed);
        } else {
            complete = Some(observed);
        }
    }
    complete.ok_or_else(invalid)
}

fn observe_direct_reference_custody(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    argument: &StructuralArgument,
) -> Result<ReferenceObservation, LegalizationError> {
    if argument.access == StructuralAccess::Owned {
        return Err(invalid());
    }
    let source_access = function
        .structural_parameters
        .iter()
        .chain(function.block_structural_parameters())
        .find(|parameter| parameter.place == argument.place)
        .map(|parameter| parameter.access)
        .unwrap_or(StructuralAccess::Owned);
    if source_access == StructuralAccess::WriteOnlyBorrow
        || source_access == StructuralAccess::SharedBorrow
            && argument.access != StructuralAccess::SharedBorrow
    {
        return Err(invalid());
    }
    let mut current = source_contract(function, custody, argument.place).ok_or(invalid())?;
    let mut cursors = vec![(argument.place, Vec::new(), Vec::new())];
    let cursor_budget = custody
        .leaves
        .len()
        .max(1)
        .checked_mul(argument.path.len())
        .ok_or(invalid())?;
    let mut dependencies = Vec::new();
    let mut prefix_start = 0;
    for (position, segment) in argument.path.iter().enumerate() {
        if *segment != StructuralPathSegment::Referent {
            continue;
        }
        let prefix = &argument.path[prefix_start..position];
        let reference_type = leaf_type(types, current, prefix).ok_or(invalid())?;
        let StructuralTypeShape::Reference { referent, access } =
            terminal_psi::find_structural_type(types, reference_type)
                .ok_or(invalid())?
                .shape
        else {
            return Err(invalid());
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
            return Err(invalid());
        }
        let mut origins = Vec::new();
        let mut next_cursors = Vec::new();
        let mut descriptors = Vec::new();
        let mut borrowed = Vec::new();
        let mut absent_case_descriptors = false;
        for (place, mut path, ancestors) in cursors {
            path.extend_from_slice(prefix);
            let root = source_contract(function, custody, place).ok_or(invalid())?;
            let candidates = terminal_semantics::stored_descriptor_paths(
                types.iter(),
                root,
                &path,
                cursor_budget,
            )
            .ok_or_else(invalid)?;
            for mut path in candidates {
                if source_contract(function, custody, place)
                    .and_then(|contract| leaf_type(types, contract, &path))
                    != Some(reference_type)
                {
                    return Err(invalid());
                }
                let Some(observed) =
                    super::observations::observe(function, custody, types, place, &path)
                else {
                    if !further_crossing
                        && !path.contains(&StructuralPathSegment::Referent)
                        && path
                            .iter()
                            .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
                    {
                        absent_case_descriptors = true;
                        continue;
                    }
                    return Err(invalid());
                };
                let leaf = observed
                    .resident
                    .as_ref()
                    .and_then(|slot| custody.leaves.get(slot));
                // Access concerns the receiving slot the read actually names,
                // not an alias's syntactic root. Keep the logical cursor below
                // for further crossings, but authenticate this physical carrier
                // against every live enclosing loan at the same frontier.
                let (observed_carrier_place, observed_carrier_path) = observed
                    .resident
                    .as_ref()
                    .map_or((place, path.as_slice()), |(place, path)| {
                        (*place, path.as_slice())
                    });
                if observed.referent != referent
                    || leaf.is_some_and(|leaf| {
                        custody.leaves.iter().any(|((carrier, path), child)| {
                            custody.lifetime_access_conflicts(
                                child,
                                &leaf.identity,
                                argument.access,
                            ) || custody.descends_from(child, &leaf.identity)
                                && !shared_source_compatible(
                                    function,
                                    custody,
                                    types,
                                    carrier,
                                    path,
                                    argument.access,
                                )
                        })
                    })
                {
                    return Err(invalid());
                }
                // A descriptor is stored data too. Direct access cannot bypass
                // a live enclosing loan; traversing that exact ancestor is the
                // only non-shared exemption, retained separately on each cursor.
                if custody.leaves.iter().any(|(_, enclosing)| {
                    super::lifetime_storage_access_conflicts(
                        function,
                        custody,
                        types,
                        enclosing,
                        observed_carrier_place,
                        observed_carrier_path,
                        StructuralAccess::SharedBorrow,
                    ) || !ancestors.contains(&enclosing.identity)
                        && super::leaf_storage_access_conflicts(
                            function,
                            custody,
                            types,
                            enclosing,
                            observed_carrier_place,
                            observed_carrier_path,
                            StructuralAccess::SharedBorrow,
                        )
                }) {
                    return Err(invalid());
                }
                if let Some(slot) = &observed.resident {
                    descriptors.push(slot.clone());
                }
                dependencies.push(Root::Ingress(place, path.clone()));
                if observed_carrier_place != place || observed_carrier_path != path.as_slice() {
                    dependencies.push(Root::Ingress(
                        observed_carrier_place,
                        observed_carrier_path.to_vec(),
                    ));
                }
                origins.push(observed.root.clone());
                if further_crossing {
                    dependencies.push(observed.root.clone());
                    let mut ancestors = ancestors.clone();
                    ancestors.extend(observed.parents.iter().cloned());
                    // Continue the actual logical read at this frontier. A
                    // captured origin is never a request to reload its old slot.
                    path.push(StructuralPathSegment::Referent);
                    let cursor = (place, path, ancestors);
                    if !next_cursors.contains(&cursor) {
                        if next_cursors.len() >= cursor_budget {
                            return Err(invalid());
                        }
                        next_cursors.push(cursor);
                    }
                }
                if leaf.is_none() {
                    borrowed.push(observed);
                }
            }
        }
        if !further_crossing {
            // This walk establishes descriptor custody and addressability,
            // not the call's endpoint type. Inline fields retain their own
            // storage contract; the consuming call checks its presentation.
            if origins.is_empty() && !absent_case_descriptors
                || crate::structural_inputs::structural_reference_input::reference_formation_geometry(
                    referent,
                    &argument.path[position + 1..],
                    types,
                )
                .is_none()
            {
                return Err(invalid());
            }
            return Ok(ReferenceObservation {
                origins,
                referent,
                dependencies,
                descriptors,
                borrowed,
                absent_case_descriptors,
            });
        }
        cursors = next_cursors;
        current = referent;
        prefix_start = position + 1;
    }
    Err(invalid())
}
