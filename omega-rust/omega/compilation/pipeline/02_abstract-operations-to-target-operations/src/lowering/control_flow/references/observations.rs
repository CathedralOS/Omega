//! Current pointer contents are observations, not owned carrier residency.
//! Reuse the call-effect replay, including selected callees, at the exact
//! pre-operation frontier. Keep captured entry values symbolic: turning them
//! into slot addresses would reload replacements and invent containment.

use super::*;
use terminal_semantics::descriptor_lifetimes::{
    descriptor_location, descriptor_path, fixed_runtime_index_alternatives,
};
use terminal_semantics::descriptor_results::{DescriptorCallOrigin, DescriptorSourceOrigin};

/// An owned join can relocate a descriptor while its pointee stays in the
/// same live home. Follow the retained resident's exact possible storage to
/// observe a nested descriptor there. This transports metadata only: emitted
/// addressing still loads the pointer through the authored carrier, and no
/// origin or matching type grants access. Every possible backing must resolve
/// to current compatible contents; an opaque or retired home stays unknown.
pub(super) fn through_resident(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
    active: &mut Vec<(PlaceId, Vec<StructuralPathSegment>)>,
) -> Option<ReferenceStorage> {
    let crossing = path
        .iter()
        .position(|segment| *segment == StructuralPathSegment::Referent)?;
    let carrier =
        super::reference_leaf_following(function, live, types, place, &path[..crossing], active)?;
    let expected = leaf_type(types, source_contract(function, live, place)?, path)?;
    let mut candidates = Vec::new();
    for origin in carrier.root.origins() {
        let target_operations::TargetReferenceOriginBase::Storage { root } = origin.base else {
            return None;
        };
        let mut selected = origin.exact_path()?;
        selected.extend_from_slice(&path[crossing + 1..]);
        let resident =
            super::reference_leaf_following(function, live, types, root, &selected, active)?;
        if resident.carrier_type != expected {
            return None;
        }
        candidates.push(resident.into_owned());
    }
    combine_alternatives(function, candidates).ok()
}

pub(super) fn observe(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<ReferenceStorage> {
    let root = source_contract(function, live, place)?;
    let carrier_type = leaf_type(types, root, path)?;
    let StructuralTypeShape::Reference { referent, .. } = types.get(&carrier_type)?.shape else {
        return None;
    };
    // One static slot is one location. A runtime index over a fixed array
    // names every in-range element; each element keeps its own observed
    // descriptor, and the pointer still selects among those origins.
    let locations =
        if let Some(location) = descriptor_location(types.catalog(), place, root, path, None) {
            vec![location]
        } else {
            fixed_runtime_index_alternatives(types.catalog(), place, root, path)?
        };
    let mut candidates = Vec::with_capacity(locations.len());
    for location in locations {
        // Borrowed arrivals can resolve an alias to its physical carrier.
        // Demand-driven snapshots name the authored alias, but live residents
        // still own the physical slots. Resolve each concrete alternative
        // there before asking the source graph; never require a second
        // snapshot under an address-equivalent spelling.
        let selected_path = descriptor_path(types.catalog(), root, &location.path)?;
        if let Some(leaf) = live.references.get(&(place, selected_path)) {
            if leaf.carrier_type != carrier_type || leaf.referent != referent {
                return None;
            }
            candidates.push(leaf.clone());
            continue;
        }
        let mut origins = Vec::new();
        let current = live
            .stored_reference_effects
            .observations
            .get(&function.machine)?
            .get(&live.reference_position)?
            .get(&location)?
            .origins
            .as_ref()?;
        for source in current {
            let origin = match source {
                DescriptorSourceOrigin::Provider(provider) => {
                    let origin = live.stored_reference_effects.provider_origin(provider)?;
                    let target_operations::TargetReferenceOriginBase::Provider {
                        referent: promised,
                        access,
                        ..
                    } = &origin.base
                    else {
                        return None;
                    };
                    let StructuralTypeShape::Reference {
                        access: required, ..
                    } = types.get(&carrier_type)?.shape
                    else {
                        return None;
                    };
                    if leaf_type(types, *promised, &provider.projection.path) != Some(referent)
                        || !access_covers(*access, required)
                    {
                        return None;
                    }
                    origin
                }
                DescriptorSourceOrigin::IncomingSlot(slot) => {
                    let contract = function.structural_parameter(slot.root)?.structural_type;
                    let slot_path = descriptor_path(types.catalog(), contract, &slot.path)?;
                    if leaf_type(types, contract, &slot_path) != Some(carrier_type) {
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
                DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                    source,
                    projection,
                }) if projection.exact
                    && !projection.indirect
                    && projection.window.is_none()
                    && projection.captured_at.is_none() =>
                {
                    target_operations::TargetReferenceOrigin {
                        base: target_operations::TargetReferenceOriginBase::Storage {
                            root: *source,
                        },
                        path: projection
                            .path
                            .iter()
                            .cloned()
                            .map(terminal_psi::ReferenceOriginProjection::Exact)
                            .collect(),
                        extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                    }
                }
                // Unknown geometry remains unavailable, never an entry-type fallback.
                _ => return None,
            };
            origins.push(origin);
        }
        if origins.is_empty() {
            return None;
        }
        origins.sort();
        origins.dedup();
        candidates.push(ReferenceStorage {
            root: ReferenceRoot::Alternatives(origins),
            carrier_type,
            referent,
            placed: None,
        });
    }
    // An incomplete slot or incompatible interpretation declines the whole
    // observation. This joins possible addresses, not permissions.
    combine_alternatives(function, candidates).ok()
}
