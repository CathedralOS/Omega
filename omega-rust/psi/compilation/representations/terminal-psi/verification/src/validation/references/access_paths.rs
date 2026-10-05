//! Nonescaping reads and call paths join typed crossings to existing loan custody.
//! No path walk mints a permission or broadens escaping reference formation.

use semantic_vocabulary::ScalarType;

use super::{
    LiveReference, ModuleError, PlaceId, ReferenceIdentity, ReferenceOrigin, ReferenceParent,
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule, access_covers, invalid,
    leaf_reference_type, origin_root, origins_overlap, shared_source_compatible, source_type,
};

/// Resolve a nonescaping storage access without manufacturing a new loan.
/// Live traversal separately authenticates each crossed descriptor and origin.
pub(in crate::validation) fn access_source_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
    allow_referent_subtree: bool,
) -> Option<StructuralTypeId> {
    if !allow_referent_subtree || !source.path.contains(&StructuralPathSegment::Referent) {
        return source_type(module, machine, source);
    }
    // Calls retain exact static argument paths. Dynamic observation does not
    // broaden source formation or the synchronous structural-call contract.
    if source
        .path
        .iter()
        .any(|segment| segment.runtime_index().is_some())
    {
        return None;
    }
    let (current, prefix_start) = access_referent_tail(module, machine, source)?;
    leaf_reference_type(module, current, &source.path[prefix_start..])
}

/// A scalar observation validates its complete projection independently of
/// reference custody. Inline scalar fields need no fabricated structural type,
/// and a crossing or selector before the final pointer region is retained.
pub(in crate::validation) fn access_read_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> Option<ScalarType> {
    if source.access != StructuralAccess::SharedBorrow
        || !source.path.contains(&StructuralPathSegment::Referent)
    {
        return None;
    }
    access_referent_tail(module, machine, source)?;
    let signature =
        super::super::structural::result_contracts::source_signature(machine, source.place)?;
    terminal_semantics::primitive_read_projection_type(
        module.structural_types.iter(),
        signature.structural_type,
        &source.path,
    )
}

/// Authenticate permission at every pointer crossing, leaving endpoint geometry
/// to the operation that owns it. Runtime selectors determine a descriptor's
/// type here; live traversal below independently requires every possible slot.
fn access_referent_tail(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
) -> Option<(StructuralTypeId, usize)> {
    if source.access == StructuralAccess::Owned {
        return None;
    }
    let signature =
        super::super::structural::result_contracts::source_signature(machine, source.place)?;
    if signature.multiplicity == StructuralMultiplicity::Linear
        || !signature.qualifications.is_empty()
        || !signature.projected_qualifications.is_empty()
    {
        return None;
    }
    let mut current = signature.structural_type;
    let mut authority = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)
        .or_else(|| super::super::block_views::parameter(machine, source.place))
        .map_or(StructuralAccess::Owned, |parameter| parameter.access);
    let mut prefix_start = 0;
    for (position, segment) in source.path.iter().enumerate() {
        match segment {
            StructuralPathSegment::Field(_)
            | StructuralPathSegment::FixedIndex(_)
            | StructuralPathSegment::RuntimeIndex { .. } => {}
            StructuralPathSegment::Referent => {
                current = terminal_semantics::runtime_structural_path_tip(
                    module.structural_types.iter(),
                    current,
                    &source.path[prefix_start..position],
                )?
                .id;
                let (referent, access) = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == current)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::Reference { referent, access } => {
                            Some((referent, access))
                        }
                        _ => None,
                    })?;
                // Reading a pointer word requires readable storage, even when
                // the final call requests write-only access to its referent.
                if authority == StructuralAccess::WriteOnlyBorrow {
                    return None;
                }
                authority = if access_covers(authority, access) {
                    access
                } else if access_covers(access, authority) {
                    authority
                } else {
                    return None;
                };
                current = referent;
                prefix_start = position + 1;
            }
            _ => return None,
        }
    }
    if !access_covers(authority, source.access) {
        return None;
    }
    Some((current, prefix_start))
}

/// An observation or synchronous borrow retains every final origin and protects the
/// descriptor storage read along the way. It creates no persistent child loan.
pub(super) struct AccessReferenceCustody {
    pub(super) origins: Vec<ReferenceOrigin>,
    pub(super) dependencies: Vec<ReferenceOrigin>,
}

pub(super) fn access_reference_custody(
    module: &TerminalModule,
    machine: &TerminalMachine,
    live: &[LiveReference],
    argument: &StructuralArgument,
) -> Result<AccessReferenceCustody, ModuleError> {
    let invalid_path = || {
        invalid(
            machine,
            "reference access path exceeds its exact type or access",
        )
    };
    let invalid_live = || {
        if std::env::var_os("OMEGA_DEBUG_REFERENCE_RETIREMENT").is_some() {
            eprintln!(
                "retirement invalid reference access machine={:?} argument={:?} live={:?}",
                machine.id, argument, live
            );
        }
        invalid(
            machine,
            "reference access descriptor is absent, ambiguous or suspended",
        )
    };
    let invalid_opaque = || {
        invalid(
            machine,
            "reference access crossing has no exact stored lineage",
        )
    };
    // Call argument validation and primitive read validation own the final
    // endpoint. This transaction authenticates crossed descriptor custody;
    // it cannot manufacture geometry or a new reference formation.
    access_referent_tail(module, machine, argument).ok_or_else(invalid_path)?;
    let mut current =
        super::super::structural::result_contracts::source_signature(machine, argument.place)
            .ok_or_else(|| invalid(machine, "reference access root is absent"))?
            .structural_type;
    let mut cursors = vec![(argument.place, Vec::new(), Vec::new())];
    let cursor_budget = live
        .len()
        .checked_mul(argument.path.len())
        .ok_or_else(invalid_opaque)?;
    let mut dependencies = Vec::new();
    let mut prefix_start = 0;
    for (position, segment) in argument.path.iter().enumerate() {
        if !matches!(segment, StructuralPathSegment::Referent) {
            continue;
        }
        let prefix = &argument.path[prefix_start..position];
        let reference_type = terminal_semantics::runtime_structural_path_tip(
            module.structural_types.iter(),
            current,
            prefix,
        )
        .ok_or_else(invalid_path)?
        .id;
        let referent = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == reference_type)
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::Reference { referent, .. } => Some(referent),
                _ => None,
            })
            .ok_or_else(invalid_path)?;
        let further_crossing =
            argument.path[position + 1..].contains(&StructuralPathSegment::Referent);
        let mut origins = Vec::new();
        let mut next_cursors = Vec::new();
        for (place, mut selected_path, ancestors) in cursors {
            selected_path.extend_from_slice(prefix);
            let root_type =
                super::super::structural::result_contracts::source_signature(machine, place)
                    .ok_or_else(invalid_path)?
                    .structural_type;
            let candidates =
                descriptor_candidates(module, machine, root_type, &selected_path, live.len())
                    .ok_or_else(invalid_live)?;
            for path in candidates {
                let actual =
                    super::super::structural::result_contracts::source_signature(machine, place)
                        .and_then(|signature| {
                            leaf_reference_type(module, signature.structural_type, &path)
                        })
                        .ok_or_else(invalid_path)?;
                if actual != reference_type {
                    return Err(invalid_path());
                }
                if !descriptor_storage_available(
                    module,
                    machine,
                    live,
                    place,
                    &path,
                    &ancestors,
                    argument.access,
                ) {
                    return Err(invalid_live());
                }
                let mut matching = live.iter().filter(|reference| {
                    reference.carrier == place && reference.carrier_path == path
                });
                let carrier = matching.next().ok_or_else(invalid_live)?;
                if matching.next().is_some()
                    || live.iter().any(|child| {
                        matches!(&child.parent,
                            ReferenceParent::Reference(parent) if parent == &carrier.identity)
                            && !shared_source_compatible(module, machine, child, argument.access)
                    })
                {
                    return Err(invalid_live());
                }
                dependencies.push(ReferenceOrigin::IngressLeaf(ReferenceIdentity {
                    place,
                    path,
                }));
                origins.push(carrier.root.clone());
                if further_crossing {
                    dependencies.push(carrier.root.clone());
                    let candidates = match &carrier.root {
                        ReferenceOrigin::Primitive(place) => vec![(*place, Vec::new())],
                        ReferenceOrigin::IngressLeaf(identity) => {
                            vec![(identity.place, identity.path.clone())]
                        }
                        ReferenceOrigin::Alternatives { place, paths } => {
                            paths.iter().map(|path| (*place, path.clone())).collect()
                        }
                    };
                    for (place, path) in candidates {
                        // An ingress leaf can name an opaque permission rather
                        // than concrete storage. Only an exact referent contract
                        // may become the cursor for another stored descriptor.
                        if super::super::structural::result_contracts::source_signature(
                            machine, place,
                        )
                        .and_then(|signature| {
                            leaf_reference_type(module, signature.structural_type, &path)
                        }) != Some(referent)
                        {
                            return Err(invalid_opaque());
                        }
                        let mut ancestry = ancestors.clone();
                        ancestry.push(carrier.identity.clone());
                        if !next_cursors.contains(&(place, path.clone(), ancestry.clone())) {
                            if next_cursors.len() >= cursor_budget {
                                return Err(invalid_opaque());
                            }
                            next_cursors.push((place, path, ancestry));
                        }
                    }
                }
            }
        }
        if !further_crossing {
            if origins.is_empty() {
                return Err(invalid_live());
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
    Err(invalid_path())
}

/// Expand the selected descriptor slots from declarations, never from the live
/// matches: otherwise an omitted permission would silently shrink the candidate
/// set. Every possible slot needs a live row, so the roster is also a natural
/// allocation bound. Runtime view extents supply no fixed owned leaf roster.
fn descriptor_candidates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    mut current: StructuralTypeId,
    path: &[StructuralPathSegment],
    maximum: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
    if maximum == 0 {
        return None;
    }
    let mut candidates = vec![Vec::new()];
    for segment in path {
        let resolved = match segment {
            StructuralPathSegment::RuntimeIndex { index, .. } => {
                terminal_semantics::constant_runtime_index(machine, *index)
                    .map(StructuralPathSegment::FixedIndex)
            }
            _ => None,
        };
        let segment = resolved.as_ref().unwrap_or(segment);
        if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
            let declaration = terminal_semantics::runtime_structural_path_tip(
                module.structural_types.iter(),
                current,
                &[],
            )?;
            let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
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
            current = leaf_reference_type(module, current, std::slice::from_ref(segment))?;
            for candidate in &mut candidates {
                candidate.push(segment.clone());
            }
        }
    }
    Some(candidates)
}

/// A live loan over descriptor storage constrains use of its contained
/// permission. Traversing that very loan supplies authority; observing the
/// descriptor directly cannot bypass it. Opaque ingress identities are not
/// concrete storage coordinates merely because their spelling is similar.
pub(super) fn descriptor_storage_available(
    module: &TerminalModule,
    machine: &TerminalMachine,
    live: &[LiveReference],
    place: PlaceId,
    path: &[StructuralPathSegment],
    ancestors: &[ReferenceIdentity],
    requested: StructuralAccess,
) -> bool {
    let storage = ReferenceOrigin::IngressLeaf(ReferenceIdentity {
        place,
        path: path.to_vec(),
    });
    live.iter().all(|loan| {
        if origin_root(&loan.root) != place
            || ancestors.contains(&loan.identity)
            || shared_source_compatible(module, machine, loan, requested)
        {
            return true;
        }
        let Some(reference_type) =
            super::super::structural::result_contracts::source_signature(machine, loan.carrier)
                .and_then(|signature| {
                    leaf_reference_type(module, signature.structural_type, &loan.carrier_path)
                })
        else {
            return false;
        };
        let Some(referent) = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == reference_type)
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::Reference { referent, .. } => Some(referent),
                _ => None,
            })
        else {
            return false;
        };
        let conflicts = |root_place, root_path: &[StructuralPathSegment]| {
            super::super::structural::result_contracts::source_signature(machine, root_place)
                .and_then(|signature| {
                    leaf_reference_type(module, signature.structural_type, root_path)
                })
                == Some(referent)
                && origins_overlap(
                    &ReferenceOrigin::IngressLeaf(ReferenceIdentity {
                        place: root_place,
                        path: root_path.to_vec(),
                    }),
                    &storage,
                )
        };
        !match &loan.root {
            ReferenceOrigin::Primitive(place) => conflicts(*place, &[]),
            ReferenceOrigin::IngressLeaf(identity) => conflicts(identity.place, &identity.path),
            ReferenceOrigin::Alternatives { place, paths } => {
                paths.iter().any(|path| conflicts(*place, path))
            }
        }
    })
}

/// A sibling can consume or mutate descriptor storage independently of the
/// referent origins carried inside it. Dereferencing actuals only read their
/// initial descriptors; their referent accesses are compared separately.
pub(super) fn call_dependency_conflict(
    dependencies: &[ReferenceOrigin],
    argument: &StructuralArgument,
) -> bool {
    if argument.access == StructuralAccess::SharedBorrow
        || argument.path.contains(&StructuralPathSegment::Referent)
    {
        return false;
    }
    let storage = ReferenceOrigin::IngressLeaf(ReferenceIdentity {
        place: argument.place,
        path: argument.path.clone(),
    });
    dependencies
        .iter()
        .any(|dependency| origins_overlap(dependency, &storage))
}
