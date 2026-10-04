//! Nonescaping call paths join exact typed crossings to existing loan custody.
//! No path walk mints a permission or broadens escaping reference formation.

use super::{
    LiveReference, ModuleError, PlaceId, ReferenceIdentity, ReferenceOrigin, ReferenceParent,
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule, access_covers, invalid,
    leaf_reference_type, origin_root, origins_overlap, shared_source_compatible, source_type,
};

/// Resolve a nonescaping call path without manufacturing a new loan.
/// Live traversal separately authenticates each crossed descriptor and origin.
pub(in crate::validation) fn call_source_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &StructuralArgument,
    allow_referent_subtree: bool,
) -> Option<StructuralTypeId> {
    if !allow_referent_subtree || !source.path.contains(&StructuralPathSegment::Referent) {
        return source_type(module, machine, source);
    }
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
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_) => {}
            StructuralPathSegment::Referent => {
                current =
                    leaf_reference_type(module, current, &source.path[prefix_start..position])?;
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
    leaf_reference_type(module, current, &source.path[prefix_start..])
}

/// A synchronous borrow retains every possible final origin and protects the
/// descriptor storage read along the way. It creates no persistent child loan.
pub(super) struct CallReferenceCustody {
    pub(super) origins: Vec<ReferenceOrigin>,
    pub(super) dependencies: Vec<ReferenceOrigin>,
}

pub(super) fn call_reference_custody(
    module: &TerminalModule,
    machine: &TerminalMachine,
    live: &[LiveReference],
    argument: &StructuralArgument,
) -> Result<CallReferenceCustody, ModuleError> {
    let invalid_path = || {
        invalid(
            machine,
            "reference call path exceeds its exact type or access",
        )
    };
    let invalid_live = || {
        invalid(
            machine,
            "reference call descriptor is absent, ambiguous or suspended",
        )
    };
    let invalid_opaque = || {
        invalid(
            machine,
            "reference call crossing has no exact stored lineage",
        )
    };
    call_source_type(module, machine, argument, true).ok_or_else(invalid_path)?;
    let mut current =
        super::super::structural::result_contracts::source_signature(machine, argument.place)
            .ok_or_else(|| invalid(machine, "reference call root is absent"))?
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
        let reference_type =
            leaf_reference_type(module, current, prefix).ok_or_else(invalid_path)?;
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
        for (place, mut path, ancestors) in cursors {
            path.extend_from_slice(prefix);
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
            let mut matching = live
                .iter()
                .filter(|reference| reference.carrier == place && reference.carrier_path == path);
            let carrier = matching.next().ok_or_else(invalid_live)?;
            if matching.next().is_some()
                || live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(parent) if parent == &carrier.identity)
                    && !shared_source_compatible(module, machine, child, argument.access))
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
                    if super::super::structural::result_contracts::source_signature(machine, place)
                        .and_then(|signature| {
                            leaf_reference_type(module, signature.structural_type, &path)
                        })
                        != Some(referent)
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
        if !further_crossing {
            if origins.is_empty() {
                return Err(invalid_live());
            }
            return Ok(CallReferenceCustody {
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
