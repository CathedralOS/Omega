use semantic_vocabulary::{IntegerSign, ScalarType, StructuralTypeId};
use terminal_psi::{
    ReferenceOriginProjection, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(crate) mod borrowed_leaves;
pub(crate) mod call_results;
pub(crate) mod memberships;
pub use borrowed_leaves::{BorrowedResultLeaf, BorrowedResultLeafKind, borrowed_result_leaves};
pub use memberships::{
    ReferenceResultMemberships, reference_call_preserved_memberships,
    reference_result_memberships_weaken,
};
pub(crate) mod descriptor_regions;
pub(crate) mod result_regions;
pub use call_results::{
    ReferenceCallContract, ReferenceCallOrigin, ReferenceCallResult, ReferenceCallSource,
    bind_public_reference_call_results, bind_reference_call_results,
};
pub use descriptor_regions::{
    descriptor_input_origin_path, reference_result_contents, reference_result_sources,
    validate_descriptor_result_regions,
};
pub use result_regions::{
    ReferenceInventory, ReferenceLeaf, UnknownReferenceContents, reference_inventory,
    reference_leaves, validate_boundary_reference_regions, validate_dynamic_reference_regions,
};
use std::slice::from_ref;

pub struct ReferenceOriginBinding<'origin> {
    pub source: terminal_psi::StructuralArgument,
    pub suffix: &'origin [ReferenceOriginProjection],
    pub endpoint: StructuralTypeId,
    pub extent: semantic_vocabulary::ReferenceOriginExtent,
}

impl ReferenceOriginBinding<'_> {
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
                    && crate::runtime_structural_path_tip(types, root, prefix).is_some_and(
                        |row| matches!(row.shape, StructuralTypeShape::Reference { .. }),
                    )
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
        || captured_reference_origin_type(types.clone(), root, &actual.path).is_none()
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
            for (position, projection) in
                actual.path.iter().enumerate().skip(allowed.path.len())
            {
                if !matches!(
                    projection,
                    Exact(
                        StructuralPathSegment::Referent | StructuralPathSegment::FixedIndex(_)
                    ) | Element
                ) {
                    continue;
                }
                let Some(endpoint) = captured_reference_origin_type(
                    types.clone(),
                    root,
                    &actual.path[..position],
                ) else {
                    return false;
                };
                let Some(declaration) = types.clone().find(|row| row.id == endpoint) else {
                    return false;
                };
                match projection {
                    Exact(StructuralPathSegment::Referent) => {
                        if position + 1 != actual.path.len()
                            || matches!(
                                declaration.shape,
                                StructuralTypeShape::Reference { .. }
                            )
                        {
                            return false;
                        }
                    }
                    _ if descriptor_region(&declaration.shape)
                        && position != allowed.path.len() =>
                    {
                        return false;
                    }
                    _ => {}
                }
            }
            !(actual.extent == WithinRegion
                && actual.path.len() > allowed.path.len()
                && captured_reference_origin_type(types.clone(), root, &actual.path)
                    .and_then(|endpoint| crate::structural_declaration(types.clone(), endpoint))
                    .is_some_and(|row| descriptor_region(&row.shape)))
        }
    }
}

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
        let Some(collection) =
            captured_reference_origin_type(types.clone(), root, &actual.path[..position])
                .and_then(|identity| crate::structural_declaration(types.clone(), identity))
        else {
            return false;
        };
        let StructuralTypeShape::FixedArray {
            length: terminal_psi::ArrayExtent::Literal(length),
            ..
        } = collection.shape
        else {
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

pub fn reference_result_refines_regions<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    result: &terminal_psi::StructuralResultDeclaration,
    regions: &[terminal_psi::TerminalDynamicReferenceRegion],
) -> bool {
    result.reference_sources.iter().all(|mapping| {
        !mapping.sources.is_empty()
            && mapping.sources.iter().all(|source| {
                let source = match source {
                    terminal_psi::StructuralReferenceOrigin::Parameter(source) => source,
                    terminal_psi::StructuralReferenceOrigin::Invocation { carrier_type, path, extent, access } => {
                        let Some(endpoint) = reference_origin_type(types.clone(), *carrier_type, path) else {
                            return false;
                        };
                        return *extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                            && regions.iter().any(|region| {
                                let terminal_psi::TerminalDynamicReferenceOrigin::Invocation { carrier_type: allowed } = region.origin else { return false; };
                                region.path == mapping.path && region.access == *access
                                    && reference_origin_type(types.clone(), allowed, &region.source_path.iter().cloned().map(ReferenceOriginProjection::Exact).collect::<Vec<_>>()) == Some(endpoint)
                            });
                    }
                };
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
                        matches!(region.origin, terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index: index } if index as usize == argument_index)
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
                if !crate::structural_paths_may_overlap(from_ref(left), from_ref(right)) =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

pub fn captured_reference_origin_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[ReferenceOriginProjection],
) -> Option<StructuralTypeId> {
    let geometry = path
        .iter()
        .map(|projection| match projection {
            ReferenceOriginProjection::Exact(StructuralPathSegment::RuntimeIndex {
                ..
            }) => ReferenceOriginProjection::Element,
            _ => projection.clone(),
        })
        .collect::<Vec<_>>();
    resolve_reference_origin_type(types, root, &geometry)
}

pub fn reference_origin_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[ReferenceOriginProjection],
) -> Option<StructuralTypeId> {
    if path.iter().any(|projection| {
        matches!(
            projection,
            ReferenceOriginProjection::Exact(
                StructuralPathSegment::RuntimeIndex { .. }
                    | StructuralPathSegment::FixedRange { .. }
            )
        )
    }) {
        return None;
    }
    resolve_reference_origin_type(types, root, path)
}

fn resolve_reference_origin_type<'types>(
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
                let collection = crate::structural_declaration(types.clone(), root)?;
                if matches!(collection.shape, StructuralTypeShape::FixedArray { length: terminal_psi::ArrayExtent::Literal(length), .. } if *index >= length)
                {
                    return None;
                }
                root = collection_element_type(types.clone(), root)?;
            }
            ReferenceOriginProjection::Exact(segment) => {
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

pub fn collection_element_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let declaration = crate::structural_declaration(types.clone(), collection)?;
    match declaration.shape {
        StructuralTypeShape::FixedArray { element, .. }
        | StructuralTypeShape::ElementView { element, .. } => Some(element),
        StructuralTypeShape::ByteSequence(_) => {
            let mut matching = types.filter(|declaration| declaration.scalar_restriction.is_none()
                && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                    if integer.sign() == IntegerSign::Unsigned && integer.bits() == Some(8) && !integer.is_address()));
            let element = matching.next()?;
            matching.next().is_none().then_some(element.id)
        }
        _ => None,
    }
}
