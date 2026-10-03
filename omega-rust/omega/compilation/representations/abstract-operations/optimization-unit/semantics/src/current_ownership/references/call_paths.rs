//! Reconstruct nonescaping call paths from the current loan frontier.
//! Each stored pointer is a live permission; origin alternatives are never
//! fabricated descriptors or a choice of runtime addresses.

use super::{
    BlockId, LiveReference, OptimizationUnitValidationError, PlaceId, PsiOptimizationFunction,
    ReferenceIdentity, ReferenceOrigin, ReferenceParent, StructuralAccess, StructuralArgument,
    StructuralTypeDeclaration, StructuralTypeId, call_source_type, invalid, leaf_type,
    origin_place, origins_overlap, shared_source_compatible, structural_source_contract,
};
use std::collections::BTreeMap;

/// A synchronous borrow retains every possible final origin and protects the
/// descriptor storage read along the way. It creates no persistent child loan.
pub(super) struct CallReferenceCustody {
    pub(super) origins: Vec<ReferenceOrigin>,
    pub(super) dependencies: Vec<ReferenceOrigin>,
}

pub(super) fn call_reference_custody(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    argument: &StructuralArgument,
) -> Result<CallReferenceCustody, OptimizationUnitValidationError> {
    let invalid_path = || {
        invalid(
            function,
            block,
            Some(node),
            "reference call path exceeds its exact type or access",
        )
    };
    let invalid_live = || {
        invalid(
            function,
            block,
            Some(node),
            "reference call descriptor is absent, ambiguous or suspended",
        )
    };
    let invalid_opaque = || {
        invalid(
            function,
            block,
            Some(node),
            "reference call crossing has no exact stored lineage",
        )
    };
    call_source_type(function, structural_types, argument).ok_or_else(invalid_path)?;
    let mut current = structural_source_contract(function, argument.place)
        .ok_or_else(|| invalid(function, block, Some(node), "reference call root is absent"))?
        .structural_type;
    let mut cursors = vec![(argument.place, Vec::new(), Vec::new())];
    let cursor_budget = live
        .len()
        .checked_mul(argument.path.len())
        .ok_or_else(invalid_opaque)?;
    let mut dependencies = Vec::new();
    let mut prefix_start = 0;
    for (position, segment) in argument.path.iter().enumerate() {
        if !matches!(segment, terminal_psi::StructuralPathSegment::Referent) {
            continue;
        }
        let prefix = &argument.path[prefix_start..position];
        let reference_type =
            leaf_type(structural_types, current, prefix).ok_or_else(invalid_path)?;
        let referent = structural_types
            .get(&reference_type)
            .and_then(|declaration| match declaration.shape {
                terminal_psi::StructuralTypeShape::Reference { referent, .. } => Some(referent),
                _ => None,
            })
            .ok_or_else(invalid_path)?;
        let further_crossing =
            argument.path[position + 1..].contains(&terminal_psi::StructuralPathSegment::Referent);
        let mut origins = Vec::new();
        let mut next_cursors = Vec::new();
        for (place, mut path, ancestors) in cursors {
            path.extend_from_slice(prefix);
            let actual = structural_source_contract(function, place)
                .and_then(|signature| leaf_type(structural_types, signature.structural_type, &path))
                .ok_or_else(invalid_path)?;
            if actual != reference_type {
                return Err(invalid_path());
            }
            if !descriptor_storage_available(
                function,
                structural_types,
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
                    && !shared_source_compatible(function, structural_types, child, argument.access))
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
                    if structural_source_contract(function, place).and_then(|signature| {
                        leaf_type(structural_types, signature.structural_type, &path)
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
fn descriptor_storage_available(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    live: &[LiveReference],
    place: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    ancestors: &[ReferenceIdentity],
    requested: StructuralAccess,
) -> bool {
    let storage = ReferenceOrigin::IngressLeaf(ReferenceIdentity {
        place,
        path: path.to_vec(),
    });
    live.iter().all(|loan| {
        if origin_place(&loan.root) != place
            || ancestors.contains(&loan.identity)
            || shared_source_compatible(function, structural_types, loan, requested)
        {
            return true;
        }
        let Some(reference_type) =
            structural_source_contract(function, loan.carrier).and_then(|signature| {
                leaf_type(
                    structural_types,
                    signature.structural_type,
                    &loan.carrier_path,
                )
            })
        else {
            return false;
        };
        let Some(referent) = structural_types
            .get(&reference_type)
            .and_then(|declaration| match declaration.shape {
                terminal_psi::StructuralTypeShape::Reference { referent, .. } => Some(referent),
                _ => None,
            })
        else {
            return false;
        };
        let conflicts = |root_place, root_path: &[terminal_psi::StructuralPathSegment]| {
            structural_source_contract(function, root_place).and_then(|signature| {
                leaf_type(structural_types, signature.structural_type, root_path)
            }) == Some(referent)
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
        || argument
            .path
            .contains(&terminal_psi::StructuralPathSegment::Referent)
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
