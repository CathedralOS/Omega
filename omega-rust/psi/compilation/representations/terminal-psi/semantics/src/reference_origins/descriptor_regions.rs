//! A dynamic row must refine the requirement with its current descriptor
//! contents, not an empty pointer-reference summary. Stored incoming contents
//! retain their typed route: borrowing a record's storage does not authorize
//! the external backing of a descriptor stored inside it.

use semantic_vocabulary::ReferenceOriginExtent;
use terminal_psi::{
    ReferenceOriginProjection, ReferenceOriginSource, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralResultDeclaration, StructuralTypeDeclaration,
    TerminalDynamicReferenceRegion,
};

use crate::borrowed_storage::Projection;
use crate::descriptor_lifetimes::{descriptor_location, descriptor_location_type, descriptor_path};
use crate::descriptor_results::{
    DescriptorContentsStep, DescriptorResultSlot, DescriptorStoredOrigin,
};

/// Called at representation admission with independently replayed current row
/// bodies. Empty inactive-case slots may narrow the public case frontier; an
/// unknown slot or an origin outside every public alternative cannot.
pub fn validate_descriptor_result_regions(
    types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    result: &StructuralResultDeclaration,
    regions: &[TerminalDynamicReferenceRegion],
    slots: &[DescriptorResultSlot],
) -> Result<(), &'static str> {
    let leaves = super::borrowed_result_leaves(
        types.iter(),
        result.structural_type,
        result.multiplicity,
        regions.len(),
    )?;
    for leaf in leaves
        .into_iter()
        .filter(|leaf| matches!(leaf.kind, super::BorrowedResultLeafKind::View))
    {
        let location = descriptor_location(
            types,
            result.place,
            result.structural_type,
            &leaf.path,
            None,
        )
        .ok_or("dynamic returned view has no typed descriptor location")?;
        let mut matching = slots.iter().filter(|slot| slot.path == location.path);
        let slot = matching
            .next()
            .ok_or("dynamic row has no complete returned view origins")?;
        if matching.next().is_some() || slot.origins.is_empty() {
            return Err("dynamic row has ambiguous or unknown returned view origins");
        }
        for origin in &slot.origins {
            if matches!(origin, DescriptorStoredOrigin::Empty) {
                continue;
            }
            let (position, path) = input_origin_path(types, parameters, origin)?;
            let parameter = parameters
                .get(position)
                .ok_or("dynamic returned view names no formal input")?;
            let actual = ReferenceOriginSource {
                place: parameter.place,
                path,
                extent: ReferenceOriginExtent::WithinRegion,
                access: leaf.access,
            };
            let allowed = regions
                .iter()
                .filter(|region| {
                    region.path == leaf.path
                        && matches!(region.origin, terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index } if usize::try_from(argument_index).ok() == Some(position))
                })
                .map(|region| ReferenceOriginSource {
                    place: parameter.place,
                    path: region
                        .source_path
                        .iter()
                        .cloned()
                        .map(ReferenceOriginProjection::Exact)
                        .collect(),
                    extent: ReferenceOriginExtent::WithinRegion,
                    access: region.access,
                })
                .collect::<Vec<_>>();
            if !super::reference_origin_is_covered(
                types.iter(),
                parameter.structural_type,
                &actual,
                &allowed,
            ) && !inline_view_is_covered(types, parameter.structural_type, &actual, &allowed)
            {
                return Err("dynamic row returned view exceeds its public input regions");
            }
        }
    }
    Ok(())
}

fn input_origin_path(
    types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    origin: &DescriptorStoredOrigin,
) -> Result<(usize, Vec<ReferenceOriginProjection>), &'static str> {
    let (position, mut path, projection) = match origin {
        DescriptorStoredOrigin::Parameter {
            position,
            projection,
        } => (*position, Vec::new(), Some(projection)),
        DescriptorStoredOrigin::IncomingSlot { position, path } => {
            let parameter = parameters
                .get(*position)
                .ok_or("returned view slot names no formal input")?;
            let path = descriptor_path(types, parameter.structural_type, path)
                .ok_or("returned view slot has no complete typed input path")?;
            (
                *position,
                path.into_iter()
                    .map(ReferenceOriginProjection::Exact)
                    .collect(),
                None,
            )
        }
        DescriptorStoredOrigin::IncomingContents {
            position,
            steps,
            projection,
        } => {
            let parameter = parameters
                .get(*position)
                .ok_or("returned view contents name no formal input")?;
            (
                *position,
                contents_path(types, parameter.structural_type, steps)?,
                Some(projection),
            )
        }
        _ => return Err("dynamic returned view has no public input-relative origin"),
    };
    if let Some(projection) = projection {
        append_projection(&mut path, projection)?;
    }
    Ok((position, path))
}

fn append_projection(
    path: &mut Vec<ReferenceOriginProjection>,
    projection: &Projection,
) -> Result<(), &'static str> {
    if projection.indirect {
        return Err("dynamic returned view has unresolved indirect backing");
    }
    for segment in &projection.path {
        match segment {
            // Windows narrow the same backing. Their scalar coordinates are
            // private to this activation and cannot become caller addresses.
            StructuralPathSegment::FixedByteRange { .. } => break,
            StructuralPathSegment::RuntimeIndex { .. } => {
                path.push(ReferenceOriginProjection::Element)
            }
            StructuralPathSegment::Field(_)
            | StructuralPathSegment::Case(_)
            | StructuralPathSegment::FixedIndex(_)
            | StructuralPathSegment::Referent => {
                path.push(ReferenceOriginProjection::Exact(segment.clone()))
            }
            _ => return Err("dynamic returned view has an unsupported backing projection"),
        }
    }
    Ok(())
}

fn contents_path(
    types: &[StructuralTypeDeclaration],
    mut root: semantic_vocabulary::StructuralTypeId,
    steps: &[DescriptorContentsStep],
) -> Result<Vec<ReferenceOriginProjection>, &'static str> {
    let mut path = Vec::new();
    let mut exact = Vec::new();
    for step in steps {
        match step {
            DescriptorContentsStep::Address(segment) => exact.push(segment.clone()),
            DescriptorContentsStep::AnyElement { element } => {
                let collection = descriptor_location_type(types, root, &exact)
                    .ok_or("returned view contents have no typed collection")?;
                if super::collection_element_type(types.iter(), collection) != Some(*element) {
                    return Err("returned view contents disagree with the input element type");
                }
                path.extend(
                    descriptor_path(types, root, &exact)
                        .ok_or("returned view contents have no typed prefix")?
                        .into_iter()
                        .map(ReferenceOriginProjection::Exact),
                );
                path.push(ReferenceOriginProjection::Element);
                exact.clear();
                root = *element;
            }
        }
    }
    path.extend(
        descriptor_path(types, root, &exact)
            .ok_or("returned view contents have no typed endpoint")?
            .into_iter()
            .map(ReferenceOriginProjection::Exact),
    );
    Ok(path)
}

fn inline_view_is_covered(
    types: &[StructuralTypeDeclaration],
    root: semantic_vocabulary::StructuralTypeId,
    actual: &ReferenceOriginSource,
    allowed: &[ReferenceOriginSource],
) -> bool {
    let Some(path) = actual
        .path
        .iter()
        .map(|segment| match segment {
            ReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
            ReferenceOriginProjection::Element => None,
        })
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let inline_view = crate::runtime_structural_field(types.iter(), root, &path)
        .and_then(|field| field.field_type.byte_sequence_carrier())
        .is_some_and(|carrier| {
            matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            )
        });
    // An inline descriptor has no standalone type identity. Equality with the
    // declared supplying field is sufficient; an enclosing record is not.
    inline_view
        && allowed.iter().any(|allowed| {
            allowed.place == actual.place
                && allowed.access == actual.access
                && allowed.path == actual.path
        })
}
