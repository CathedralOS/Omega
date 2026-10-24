//! Borrowed pointer contents are reconstructed independently of owned leaves.
//! The source graph supplies current values; declarations supply types only.
//! Observing external contents creates no resident permission occurrence.

use super::*;
use terminal_semantics::descriptor_lifetimes::{
    descriptor_location, descriptor_path, fixed_runtime_index_alternatives,
};
use terminal_semantics::descriptor_results::DescriptorSourceOrigin;

pub(super) struct Observed {
    pub(super) root: Root,
    pub(super) referent: StructuralTypeId,
    pub(super) parents: Vec<OccurrenceIdentity>,
    /// Exact current carrier for copies and permission checks. Copying this
    /// occurrence preserves its parents rather than reborrowing the carrier.
    pub(super) resident: Option<(PlaceId, Vec<StructuralPathSegment>)>,
}

/// Normalize every possible selected descriptor without merging its resident
/// identity. Calls still check each resident's permissions independently.
pub(super) fn observe_alternatives(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<Vec<Observed>> {
    let contract = source_contract(function, custody, place)?;
    if custody.leaf(place, path).is_some()
        || descriptor_location(types, place, contract, path, None).is_some()
    {
        return observe(function, custody, types, place, path).map(|observed| vec![observed]);
    }
    let locations = fixed_runtime_index_alternatives(types, place, contract, path)?;
    locations
        .iter()
        .map(|location| {
            let path = descriptor_path(types, contract, &location.path)?;
            observe(function, custody, types, place, &path)
        })
        .collect()
}

pub(super) fn observe(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<Observed> {
    observe_following(function, custody, types, place, path, &mut Vec::new())
}

fn observe_following(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
    active: &mut Vec<(PlaceId, Vec<StructuralPathSegment>)>,
) -> Option<Observed> {
    if let Some(leaf) = custody.leaf(place, path) {
        return Some(Observed {
            root: leaf.root.clone(),
            referent: leaf.referent,
            parents: vec![leaf.identity.clone()],
            resident: Some((place, path.to_vec())),
        });
    }
    let contract = source_contract(function, custody, place)?;
    let carrier = leaf_type(types, contract, path)?;
    let StructuralTypeShape::Reference { referent, .. } =
        find_structural_type(types, carrier)?.shape
    else {
        return None;
    };
    let coordinate = (place, path.to_vec());
    if active.contains(&coordinate) {
        return None;
    }
    active.push(coordinate);
    let resident = through_resident(function, custody, types, place, path, active);
    active.pop();
    if let Some(resident) = resident {
        return (resident.referent == referent).then_some(resident);
    }
    let location = descriptor_location(types, place, contract, path, None)?;
    let observation = custody
        .stored_calls
        .observations
        .get(&function.machine)?
        .get(&custody.reference_position?)?
        .get(&location)?;
    // Join the carrier slot, not the pointee. The current resident leaf owns
    // receiving identity, original formations and immediate ancestry, including
    // copies and replacements. Reconstructing these from equal backing would
    // silently detach a copied child from its still-suspended parent.
    if let Some(storage) = &observation.storage {
        let storage_type = source_contract(function, custody, storage.root)?;
        let storage_path = descriptor_path(types, storage_type, &storage.path)?;
        if leaf_type(types, storage_type, &storage_path) != Some(carrier) {
            return None;
        }
        if let Some(leaf) = custody.leaf(storage.root, &storage_path) {
            if leaf.referent != referent {
                return None;
            }
            return Some(Observed {
                root: leaf.root.clone(),
                referent,
                parents: vec![leaf.identity.clone()],
                resident: Some((storage.root, storage_path)),
            });
        }
    }
    let sources = observation.origins.as_ref()?;
    let mut origins = Vec::new();
    for source in sources {
        let origin = match source {
            DescriptorSourceOrigin::IncomingSlot(slot) => {
                let parameter = function.structural_parameter(slot.root)?;
                let slot_path = descriptor_path(types, parameter.structural_type, &slot.path)?;
                if parameter.access == StructuralAccess::Owned {
                    // An owned ingress descriptor has a real resident
                    // occurrence; its absence cannot become external custody.
                    // A slot behind that descriptor is external storage, but
                    // only the authenticated original incoming pointer may
                    // supply its incoming contents at this observation epoch.
                    authenticate_indirect_ingress(
                        function, custody, types, place, path, slot.root, &slot_path,
                    )?;
                }
                if leaf_type(types, parameter.structural_type, &slot_path) != Some(carrier) {
                    return None;
                }
                target_operations::TargetReferenceOrigin {
                    base: target_operations::TargetReferenceOriginBase::IncomingContents {
                        root: slot.root,
                        slot: slot_path,
                    },
                    path: Vec::new(),
                    extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                }
            }
            // Exact backing coordinates do not prove absence of a local
            // parent. Locally formed values require the resident join above.
            _ => return None,
        };
        origins.push(origin);
    }
    if origins.is_empty() {
        return None;
    }
    origins.sort();
    origins.dedup();
    // Traversing a containing pointer reads the slot, not a reborrow of the
    // copied pointer's pointee. Those traversal dependencies belong to the
    // operation; external slot contents have no owned local parent to invent.
    Some(Observed {
        root: Root::Alternatives(origins),
        referent,
        parents: Vec::new(),
        resident: None,
    })
}

/// Follow the completed descriptor's actual storage origin before consulting
/// symbolic incoming contents. This preserves the inner resident occurrence
/// through owned joins without manufacturing custody from its backing type.
fn through_resident(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
    active: &mut Vec<(PlaceId, Vec<StructuralPathSegment>)>,
) -> Option<Observed> {
    let crossing = path
        .iter()
        .position(|segment| *segment == StructuralPathSegment::Referent)?;
    let carrier = observe_following(function, custody, types, place, &path[..crossing], active)?;
    // Retain the ordinary prefix permission/access check, including ancestor
    // conflicts. Origin geometry alone does not authorize loading the slot.
    super::call_paths::call_reference_custody(
        function,
        custody,
        types,
        &StructuralArgument {
            place,
            path: path[..=crossing].to_vec(),
            access: StructuralAccess::SharedBorrow,
        },
    )
    .ok()?;
    let origins =
        super::storage_origins(function, custody, types, &carrier.root, carrier.referent).ok()?;
    let mut selected: Option<Observed> = None;
    for origin in origins {
        let target_operations::TargetReferenceOriginBase::Storage { root } = origin.base else {
            return None;
        };
        let mut projected = origin.exact_path()?;
        projected.extend_from_slice(&path[crossing + 1..]);
        let observed = observe_following(function, custody, types, root, &projected, active)?;
        if let Some(previous) = &selected {
            // A scalar observation cannot collapse different permission
            // occurrences merely because their referents have equal layout.
            if previous.root != observed.root
                || previous.referent != observed.referent
                || previous.parents != observed.parents
                || previous.resident != observed.resident
            {
                return None;
            }
        } else {
            selected = Some(observed);
        }
    }
    selected
}

/// Inline owned slots require resident custody above. An indirect ingress
/// slot instead requires every earlier pointer crossing to retain its exact
/// incoming provenance. The shared call walk checks readable access, current
/// occurrences, ancestor conflicts and declaration geometry; recursion here
/// strictly shortens the descriptor path at its last pointer crossing.
fn authenticate_indirect_ingress(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    current_place: PlaceId,
    current_path: &[StructuralPathSegment],
    root: PlaceId,
    incoming_path: &[StructuralPathSegment],
) -> Option<()> {
    let crossing = current_path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
    let incoming_crossing = incoming_path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
    let carrier_path = &incoming_path[..incoming_crossing];
    let observed = super::call_paths::call_reference_custody(
        function,
        custody,
        types,
        &StructuralArgument {
            place: current_place,
            path: current_path[..=crossing].to_vec(),
            access: StructuralAccess::SharedBorrow,
        },
    )
    .ok()?;
    let incoming = if carrier_path.contains(&StructuralPathSegment::Referent) {
        Root::Alternatives(vec![target_operations::TargetReferenceOrigin {
            base: target_operations::TargetReferenceOriginBase::IncomingContents {
                root,
                slot: carrier_path.to_vec(),
            },
            path: Vec::new(),
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
        }])
    } else {
        Root::Ingress(root, carrier_path.to_vec())
    };
    (!observed.origins.is_empty() && observed.origins.iter().all(|origin| *origin == incoming))
        .then_some(())
}
