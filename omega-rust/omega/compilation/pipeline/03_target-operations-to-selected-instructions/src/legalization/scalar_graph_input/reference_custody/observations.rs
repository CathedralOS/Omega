//! Borrowed pointer contents are reconstructed independently of owned leaves.
//! The source graph supplies current values; declarations supply types only.
//! Observing external contents creates no resident permission occurrence.

use super::*;
use terminal_semantics::descriptor_lifetimes::{descriptor_location, descriptor_path};
use terminal_semantics::descriptor_results::DescriptorSourceOrigin;

pub(super) struct Observed {
    pub(super) root: Root,
    pub(super) referent: StructuralTypeId,
    pub(super) parents: Vec<OccurrenceIdentity>,
    /// Exact current carrier for copies and permission checks. Copying this
    /// occurrence preserves its parents rather than reborrowing the carrier.
    pub(super) resident: Option<(PlaceId, Vec<StructuralPathSegment>)>,
}

pub(super) fn observe(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
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
                if parameter.access == StructuralAccess::Owned {
                    // An owned ingress descriptor has a real resident
                    // occurrence; its absence cannot become external custody.
                    return None;
                }
                let slot_path = descriptor_path(types, parameter.structural_type, &slot.path)?;
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
