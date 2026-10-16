//! Type geometry for returned provenance. Element families describe a possible
//! subobject, never an evaluated selector or permission to access one. Keep the
//! executable path walkers unchanged so a catalog cannot supply an address.

use semantic_vocabulary::{IntegerSign, ScalarType, StructuralTypeId};
use terminal_psi::{
    ReferenceOriginProjection, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

mod borrowed_leaves;
mod call_results;
pub use borrowed_leaves::{BorrowedResultLeaf, BorrowedResultLeafKind, borrowed_result_leaves};
mod descriptor_regions;
mod result_regions;
pub use call_results::{
    ReferenceCallContract, ReferenceCallResult, ReferenceCallSource, bind_reference_call_results,
};
pub use descriptor_regions::validate_descriptor_result_regions;
pub use result_regions::{ReferenceLeaf, reference_leaves, validate_dynamic_reference_regions};

/// The exact supplying permission and the relation below it. All consumers use
/// this split; none evaluate a callee's selector in the caller's value namespace.
pub struct ReferenceOriginBinding<'origin> {
    pub source: terminal_psi::StructuralArgument,
    pub suffix: &'origin [ReferenceOriginProjection],
    pub endpoint: StructuralTypeId,
    pub extent: semantic_vocabulary::ReferenceOriginExtent,
}

impl ReferenceOriginBinding<'_> {
    /// Only a projected stored descriptor can transfer an existing carrier.
    /// Regions and element families form descendants even with an empty suffix.
    pub fn transfers_existing<'types>(
        &self,
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    ) -> bool {
        self.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
            && self.suffix.is_empty()
            && matches!(
                access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::SharedBorrow
            )
            && self.source.path.split_last().is_some_and(|(last, prefix)| {
                *last == StructuralPathSegment::Referent
                    && crate::runtime_structural_path_tip(types, root, prefix).is_some_and(|row| {
                        matches!(row.shape, StructuralTypeShape::Reference { .. })
                    })
            })
    }
}

pub fn bind_reference_origin<'types, 'origin>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    origin: &'origin terminal_psi::ReferenceOriginSource,
) -> Result<ReferenceOriginBinding<'origin>, &'static str> {
    use ReferenceOriginProjection::{Element, Exact};
    use StructuralPathSegment::{FixedIndex, Referent};
    let endpoint = reference_origin_type(types.clone(), root, &origin.path)
        .ok_or("returned origin has no typed endpoint")?;
    let selection = origin
        .path
        .iter()
        .position(|projection| matches!(projection, Element | Exact(FixedIndex(_))));
    let last_crossing = origin
        .path
        .iter()
        .rposition(|projection| *projection == Exact(Referent));
    let split = selection.map_or(origin.path.len(), |selection| {
        last_crossing.map_or(selection, |crossing| selection.max(crossing + 1))
    });
    let prefix = origin.path[..split]
        .iter()
        .map(|projection| match projection {
            Exact(segment) => Ok(segment.clone()),
            Element => Err("returned origin needs a captured stored-descriptor identity"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source = terminal_psi::StructuralArgument {
        place: origin.place,
        path: crate::reference_source_storage_path(types, root, &prefix).to_vec(),
        access: origin.access,
    };
    Ok(ReferenceOriginBinding {
        source,
        suffix: &origin.path[split..],
        endpoint,
        extent: origin.extent,
    })
}

/// A private return may narrow a public input region, but cannot gain a
/// pointee merely because its descriptor is stored inside that region.
/// This compares admitted provenance coordinates, not runtime footprints:
/// formation and completion still authenticate bounds, codecs and lineage.
fn reference_origin_refines<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    actual: &terminal_psi::ReferenceOriginSource,
    allowed: &terminal_psi::ReferenceOriginSource,
) -> bool {
    use ReferenceOriginProjection::{Element, Exact};
    use semantic_vocabulary::ReferenceOriginExtent::{Projected, WithinRegion};
    if actual.place != allowed.place
        || actual.access != allowed.access
        || reference_origin_type(types.clone(), root, &actual.path).is_none()
        || reference_origin_type(types.clone(), root, &allowed.path).is_none()
    {
        return false;
    }
    let prefix_matches = actual.path.len() >= allowed.path.len()
        && allowed
            .path
            .iter()
            .zip(&actual.path)
            .all(|(expected, observed)| {
                expected == observed
                    || matches!(
                        (expected, observed),
                        (Element, Exact(StructuralPathSegment::FixedIndex(_)))
                    )
            });
    if !prefix_matches {
        return false;
    }
    match allowed.extent {
        Projected => actual.extent == Projected && actual.path.len() == allowed.path.len(),
        WithinRegion => {
            for (position, projection) in actual.path.iter().enumerate().skip(allowed.path.len()) {
                if !matches!(
                    projection,
                    Exact(StructuralPathSegment::Referent | StructuralPathSegment::FixedIndex(_))
                        | Element
                ) {
                    continue;
                }
                let Some(endpoint) =
                    reference_origin_type(types.clone(), root, &actual.path[..position])
                else {
                    return false;
                };
                let Some(declaration) = types.clone().find(|row| row.id == endpoint) else {
                    return false;
                };
                match projection {
                    Exact(StructuralPathSegment::Referent) => {
                        // An ending marker at ordinary storage is not a pointer
                        // crossing. Every real crossing needs its own input region.
                        if position + 1 != actual.path.len()
                            || matches!(declaration.shape, StructuralTypeShape::Reference { .. })
                        {
                            return false;
                        }
                    }
                    _ if descriptor_region(&declaration.shape)
                        && position != allowed.path.len() =>
                    {
                        // Indexing a stored view crosses into external backing,
                        // unlike indexing an inline fixed array. Only a public
                        // region selecting that view already supplies its backing.
                        return false;
                    }
                    _ => {}
                }
            }
            if actual.extent == WithinRegion
                && actual.path.len() > allowed.path.len()
                && reference_origin_type(types.clone(), root, &actual.path)
                    .and_then(|endpoint| types.clone().find(|row| row.id == endpoint))
                    .is_some_and(|row| descriptor_region(&row.shape))
            {
                return false;
            }
            true
        }
    }
}

/// Check one captured origin against the union of the declared alternatives.
/// An element family of a fixed array can be covered by its complete indexed
/// roster even when no single alternative covers the family. Split only on
/// indices already present in that roster; never enumerate storage to form an
/// address or infer permission from an equal physical footprint.
pub fn reference_origin_is_covered<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    actual: &terminal_psi::ReferenceOriginSource,
    allowed: &[terminal_psi::ReferenceOriginSource],
) -> bool {
    if allowed
        .iter()
        .any(|source| reference_origin_refines(types.clone(), root, actual, source))
    {
        return true;
    }
    for (position, projection) in actual.path.iter().enumerate() {
        if *projection != ReferenceOriginProjection::Element {
            continue;
        }
        let Some(collection) = reference_origin_type(types.clone(), root, &actual.path[..position])
            .and_then(|identity| types.clone().find(|declaration| declaration.id == identity))
        else {
            return false;
        };
        let StructuralTypeShape::FixedArray { length, .. } = collection.shape else {
            continue;
        };
        if length == 0 {
            return false;
        }
        let mut indices = allowed
            .iter()
            .filter(|source| source.place == actual.place && source.access == actual.access)
            .filter_map(|source| match source.path.get(position) {
                Some(ReferenceOriginProjection::Exact(StructuralPathSegment::FixedIndex(
                    index,
                ))) if *index < length => Some(*index),
                _ => None,
            })
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        if u64::try_from(indices.len()).ok() != Some(length) {
            continue;
        }
        let mut selected = actual.clone();
        return indices.into_iter().all(|index| {
            selected.path[position] =
                ReferenceOriginProjection::Exact(StructuralPathSegment::FixedIndex(index));
            reference_origin_is_covered(types.clone(), root, &selected, allowed)
        });
    }
    false
}

fn descriptor_region(shape: &StructuralTypeShape) -> bool {
    matches!(
        shape,
        StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            )
    )
}

/// Normalize private formal places to the structural argument lane before
/// comparing a realization with the requirement. Public alternatives describe
/// possible sources; a row returning only an empty case need not exercise them.
pub fn reference_result_refines_regions<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    result: &terminal_psi::StructuralResultDeclaration,
    regions: &[terminal_psi::TerminalDynamicReferenceRegion],
) -> bool {
    result.reference_sources.iter().all(|mapping| {
        !mapping.sources.is_empty()
            && mapping.sources.iter().all(|source| {
                let Some((argument_index, parameter)) = parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.place == source.place)
                else {
                    return false;
                };
                let allowed = regions
                    .iter()
                    .filter(|region| {
                        region.argument_index as usize == argument_index
                            && region.path == mapping.path
                    })
                    .map(|region| terminal_psi::ReferenceOriginSource {
                        place: parameter.place,
                        path: region
                            .source_path
                            .iter()
                            .cloned()
                            .map(ReferenceOriginProjection::Exact)
                            .collect(),
                        extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
                        access: region.access,
                    })
                    .collect::<Vec<_>>();
                reference_origin_is_covered(
                    types.clone(),
                    parameter.structural_type,
                    source,
                    &allowed,
                )
            })
    })
}

/// Conservative aliasing between provenance coordinates. Families can overlap
/// any element; a view or differing overlay case cannot establish disjointness.
/// These paths are never used to perform an executable projection.
pub fn reference_origin_paths_may_overlap(
    left: &[ReferenceOriginProjection],
    right: &[ReferenceOriginProjection],
) -> bool {
    use ReferenceOriginProjection::{Element, Exact};
    for (left, right) in left.iter().zip(right) {
        match (left, right) {
            (Exact(StructuralPathSegment::RepresentationView { .. }), _)
            | (_, Exact(StructuralPathSegment::RepresentationView { .. })) => return true,
            (
                Exact(StructuralPathSegment::Case(left)),
                Exact(StructuralPathSegment::Case(right)),
            ) if left != right => return true,
            (Element, _) | (_, Element) => {}
            (Exact(left), Exact(right))
                if !crate::structural_paths_may_overlap(
                    std::slice::from_ref(left),
                    std::slice::from_ref(right),
                ) =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

/// Resolve live provenance that may still carry a captured runtime selector.
/// Forget only its value for type geometry, never its formation or address.
/// Declaration decoding uses the stricter `reference_origin_type` instead.
pub fn captured_reference_origin_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[ReferenceOriginProjection],
) -> Option<StructuralTypeId> {
    let geometry = path
        .iter()
        .map(|projection| match projection {
            ReferenceOriginProjection::Exact(StructuralPathSegment::RuntimeIndex { .. }) => {
                ReferenceOriginProjection::Element
            }
            _ => projection.clone(),
        })
        .collect::<Vec<_>>();
    reference_origin_type(types, root, &geometry)
}

/// Resolve every declaration projection and collection dimension. This grants
/// neither bounds nor residence: those belong to the captured returned loan.
pub fn reference_origin_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    mut root: StructuralTypeId,
    path: &[ReferenceOriginProjection],
) -> Option<StructuralTypeId> {
    let mut exact = Vec::new();
    for projection in path {
        match projection {
            ReferenceOriginProjection::Exact(StructuralPathSegment::FixedIndex(index)) => {
                root = exact_origin_type(types.clone(), root, &exact)?;
                exact.clear();
                let collection = types.clone().find(|declaration| declaration.id == root)?;
                if matches!(collection.shape, StructuralTypeShape::FixedArray { length, .. } if *index >= length)
                {
                    return None;
                }
                // An input view has no static length. This resolves its
                // element's type, not whether the captured index was in bounds.
                root = collection_element_type(types.clone(), root)?;
            }
            ReferenceOriginProjection::Exact(segment) => {
                if matches!(
                    segment,
                    StructuralPathSegment::RuntimeIndex { .. }
                        | StructuralPathSegment::FixedByteRange { .. }
                ) {
                    return None;
                }
                exact.push(segment.clone());
            }
            ReferenceOriginProjection::Element => {
                root = exact_origin_type(types.clone(), root, &exact)?;
                exact.clear();
                root = collection_element_type(types.clone(), root)?;
            }
        }
    }
    exact_origin_type(types, root, &exact)
}

fn exact_origin_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let path = crate::reference_source_storage_path(types.clone(), root, path);
    crate::runtime_structural_path_tip(types.clone(), root, path)
        .map(|declaration| declaration.id)
        .or_else(|| crate::primitive_projection_declaration(types, root, path))
}

/// Byte collections have an implicit u8 element declaration; ordinary arrays
/// and element views name theirs explicitly. No capacity enumeration is needed.
pub fn collection_element_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let declaration = types
        .clone()
        .find(|declaration| declaration.id == collection)?;
    match declaration.shape {
        StructuralTypeShape::FixedArray { element, .. }
        | StructuralTypeShape::ElementView { element, .. } => Some(element),
        StructuralTypeShape::ByteSequence(_) => {
            let mut matching = types.filter(|declaration| declaration.scalar_restriction.is_none()
                && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                    if integer.sign() == IntegerSign::Unsigned && integer.bits() == 8 && !integer.is_address()));
            let element = matching.next()?;
            matching.next().is_none().then_some(element.id)
        }
        _ => None,
    }
}
