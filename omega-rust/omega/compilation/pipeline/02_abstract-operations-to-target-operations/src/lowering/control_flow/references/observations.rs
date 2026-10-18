//! Current pointer contents are observations, not owned carrier residency.
//! Reuse the call-effect replay, including selected callees, at the exact
//! pre-operation frontier. Keep captured entry values symbolic: turning them
//! into slot addresses would reload replacements and invent containment.

use super::*;
use terminal_semantics::descriptor_lifetimes::{descriptor_location, descriptor_path};
use terminal_semantics::descriptor_results::{DescriptorCallOrigin, DescriptorSourceOrigin};

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
    let location = descriptor_location(types.catalog(), place, root, path, None)?;
    let current = live
        .stored_reference_effects
        .observations
        .get(&function.machine)?
        .get(&live.reference_position)?
        .get(&location)?
        .origins
        .as_ref()?;
    let mut origins = Vec::new();
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
                    base: target_operations::TargetReferenceOriginBase::Storage { root: *source },
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
    Some(ReferenceStorage {
        root: ReferenceRoot::Alternatives(origins),
        carrier_type,
        referent,
        placed: None,
    })
}
