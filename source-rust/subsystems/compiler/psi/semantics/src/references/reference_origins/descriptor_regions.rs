use semantic_vocabulary::ReferenceOriginExtent;
use terminal_psi::{
    ReferenceOriginProjection, ReferenceOriginSource, StructuralAccess,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalDynamicReferenceRegion,
};

use crate::descriptors::descriptor_lifetimes::{
    descriptor_location, descriptor_location_type, descriptor_path,
};
use crate::descriptors::descriptor_results::{
    DescriptorContentsQuery, DescriptorContentsStep, DescriptorResultSlot, DescriptorStoredOrigin,
};
use crate::storage::borrowed_storage::Projection;

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
            if matches!(origin, DescriptorStoredOrigin::Literal { .. }) {
                if leaf.access != StructuralAccess::SharedBorrow {
                    return Err("dynamic literal view exceeds immutable image access");
                }
                let mut advertised = false;
                for region in regions.iter().filter(|region| region.path == leaf.path) {
                    let terminal_psi::TerminalDynamicReferenceOrigin::Parameter {
                        argument_index,
                    } = region.origin
                    else {
                        return Err(
                            "dynamic literal view has no shared input-region contract",
                        );
                    };
                    if region.access != StructuralAccess::SharedBorrow {
                        return Err("dynamic literal view exceeds its public access");
                    }
                    let parameter = usize::try_from(argument_index)
                        .ok()
                        .and_then(|position| parameters.get(position))
                        .ok_or("dynamic literal view names no formal input")?;
                    super::result_regions::validate_input_region(
                        types.iter(),
                        parameter.structural_type,
                        parameter.access,
                        &region.source_path,
                        region.access,
                    )?;
                    advertised = true;
                }
                if !advertised {
                    return Err("dynamic literal view has no public input-region contract");
                }
                continue;
            }
            let (position, path) = descriptor_input_origin_path(types, parameters, origin)?;
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
            ) && !inline_view_is_covered(
                types,
                parameter.structural_type,
                &actual,
                &allowed,
            ) {
                return Err("dynamic row returned view exceeds its public input regions");
            }
        }
    }
    Ok(())
}

/// Derive the declared input origins from actual returned descriptor
/// slots. A signature alone cannot authorize unrelated backing; callers
/// must reconstruct the slots from the body before using this mapping.
pub fn reference_result_sources(
    types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    result: &StructuralResultDeclaration,
    slots: &[DescriptorResultSlot],
) -> Result<Vec<terminal_psi::StructuralReferenceResultSource>, &'static str> {
    let leaves = super::reference_leaves(types.iter(), result.structural_type, slots.len())
        .ok_or("reference result has no complete finite leaf roster")?;
    reference_sources_for_leaves(types, parameters, result, slots, leaves)
}

pub fn reference_result_contents(
    types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    result: &StructuralResultDeclaration,
    contents: &crate::descriptor_results::DescriptorResultContents,
) -> Result<Vec<terminal_psi::StructuralReferenceResultSource>, &'static str> {
    let inventory = super::reference_inventory(
        types.iter(),
        result.structural_type,
        contents
            .slots
            .len()
            .checked_add(contents.unknown_contents.len())
            .ok_or("reference result contents count overflow")?,
    )
    .ok_or("reference result has no complete contents inventory")?;
    if !contents.unresolved_slots.is_empty() {
        let mut views = crate::descriptor_results::DescriptorSlotKind::Views
            .owned_inventory(types, result.place, result.structural_type)
            .ok_or("unresolved result slots have no complete view inventory")?;
        // A view can retain every possible input loan without choosing
        // one address. References still need a complete spatial origin
        // on every return path; unknown contents do not supply one.
        views
            .slots
            .sort_unstable_by(|left, right| left.path.cmp(&right.path));
        if contents.unresolved_slots.iter().any(|path| {
            views
                .slots
                .binary_search_by(|slot| slot.path.cmp(path))
                .is_err()
        }) {
            return Err("reference result retains an unresolved spatial origin");
        }
    }
    if inventory.unknown_contents.len() != contents.unknown_contents.len() {
        return Err("reference result omits an unknown contents packet");
    }
    let mut packets = contents.unknown_contents.iter().collect::<Vec<_>>();
    packets.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    if packets.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err("unknown result contents repeat or omit their origins");
    }
    for expected in inventory.unknown_contents {
        let mut path = crate::descriptor_lifetimes::descriptor_location(
            types,
            result.place,
            result.structural_type,
            &expected.path,
            None,
        )
        .ok_or("unknown result contents lost their exact carrier")?
        .path;
        path.push(crate::descriptor_lifetimes::DescriptorPathSegment::UnknownContents);
        let packet = packets
            .binary_search_by(|packet| packet.path.cmp(&path))
            .ok()
            .map(|position| packets[position])
            .ok_or("unknown result contents lost their custody packet")?;
        if packet.origins.is_empty() {
            if contents
                .case_facts
                .excludes_payload(result.structural_type, &packet.path)
            {
                continue;
            }
            return Err("unknown result contents repeat or omit their origins");
        }
        for origin in &packet.origins {
            if let DescriptorStoredOrigin::ProducedContents(produced) = origin {
                if produced.structural_type != expected.structural_type {
                    return Err("a call returned contents of another generic type");
                }
                continue;
            }
            let (position, mut steps) = match origin {
                DescriptorStoredOrigin::IncomingSlot { position, path } => (
                    *position,
                    path.iter()
                        .copied()
                        .map(DescriptorContentsStep::Address)
                        .collect(),
                ),
                DescriptorStoredOrigin::IncomingContents {
                    position,
                    steps,
                    projection,
                } if *projection == Projection::new(&[]) => (*position, steps.clone()),
                _ => return Err("unknown result contents need their exact admitted producer"),
            };
            if steps.pop()
                != Some(DescriptorContentsStep::Address(
                    crate::descriptor_lifetimes::DescriptorPathSegment::UnknownContents,
                ))
            {
                return Err("a reference address cannot stand for unknown value contents");
            }
            let parameter = parameters
                .get(position)
                .ok_or("unknown contents name an absent input")?;
            let query = DescriptorContentsQuery {
                root: parameter.place,
                steps,
            };
            let carrier = query
                .selected_type(types, parameter.structural_type)
                .flatten()
                .ok_or("unknown contents input has no exact carrier")?;
            if carrier != expected.structural_type {
                return Err("unknown contents changed their declared type parameter");
            }
            let declaration = terminal_psi::find_structural_type(types, carrier)
                .ok_or("unknown contents carrier is absent")?;
            // A typed element step selects an array or a view. The access check
            // below follows the view's backing permission; unrelated indirect
            // storage still needs its own origin.
            if query.exact_location().is_none()
                && (!declaration.is_copyable()
                    || !query
                        .inline_steps(types, parameter.structural_type)
                        .is_some_and(|inline| {
                            inline.iter().zip(&query.steps).all(|(inside, step)| {
                                *inside || matches!(step, DescriptorContentsStep::Element { .. })
                            })
                        }))
            {
                return Err("indexed generic contents require a copyable element with a known backing");
            }
            let path = contents_path(types, parameter.structural_type, &query.steps)?;
            let requested = if declaration.is_copyable() {
                StructuralAccess::SharedBorrow
            } else {
                StructuralAccess::MutableBorrow
            };
            if !matches!(declaration.shape, StructuralTypeShape::TypeParameter)
                || !input_path_accessible(types, parameter, &path, requested)
            {
                return Err("unknown contents were transferred without owned access");
            }
        }
    }
    reference_sources_for_leaves(
        types,
        parameters,
        result,
        &contents.slots,
        inventory.references,
    )
}

fn reference_sources_for_leaves(
    types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    result: &StructuralResultDeclaration,
    slots: &[DescriptorResultSlot],
    leaves: Vec<super::ReferenceLeaf>,
) -> Result<Vec<terminal_psi::StructuralReferenceResultSource>, &'static str> {
    let mut mappings = Vec::with_capacity(leaves.len());
    for leaf in leaves {
        let StructuralTypeShape::Reference { referent, access } = types
            .iter()
            .find(|row| row.id == leaf.structural_type)
            .ok_or("reference result leaf has no type")?
            .shape
        else {
            return Err("reference result leaf changed constructor");
        };
        let mut matching = slots.iter().filter(|slot| {
            descriptor_path(types, result.structural_type, &slot.path).as_ref()
                == Some(&leaf.path)
        });
        let slot = matching
            .next()
            .ok_or("reference result leaf has no actual return origins")?;
        if matching.next().is_some() || slot.origins.is_empty() {
            return Err("reference result origins are ambiguous or empty");
        }
        let mut sources = Vec::with_capacity(slot.origins.len());
        for origin in &slot.origins {
            if let DescriptorStoredOrigin::Provider(provider) = origin {
                // The producing call supplies this reference. Its input loans travel
                // separately in the provider's replayed lifetime dependencies.
                if provider.invocations.is_empty()
                    || provider.projection != Projection::new(&[])
                {
                    return Err("returned provider reference has no exact admitted formation");
                }
                sources.push(terminal_psi::StructuralReferenceOrigin::Invocation {
                    carrier_type: leaf.structural_type,
                    path: vec![ReferenceOriginProjection::Exact(StructuralPathSegment::Referent)],
                    extent: ReferenceOriginExtent::Projected,
                    access,
                });
                continue;
            }
            // A result known only within a declared input region,
            // such as a selected or rigid requirement call's, keeps
            // that whole region (lifetimes.md, "Returned views").
            // A result whose runtime element selector was stripped
            // keeps the exact storage after that element, with the
            // selector as `Element`, so the published origin names
            // only what the result can reach. Unresolved indirection or a
            // live runtime window has no static region to publish.
            let extent = match origin {
                DescriptorStoredOrigin::Parameter { projection, .. }
                | DescriptorStoredOrigin::IncomingContents { projection, .. }
                    if projection.indirect || projection.window.is_some() =>
                {
                    return Err(
                        "reference result needs a static retained input projection",
                    );
                }
                DescriptorStoredOrigin::Parameter { projection, .. }
                | DescriptorStoredOrigin::IncomingContents { projection, .. }
                    if !projection.exact && projection.stripped_origin_path().is_none() =>
                {
                    ReferenceOriginExtent::WithinRegion
                }
                DescriptorStoredOrigin::Parameter { .. }
                | DescriptorStoredOrigin::IncomingSlot { .. }
                | DescriptorStoredOrigin::IncomingContents { .. } => {
                    ReferenceOriginExtent::Projected
                }
                _ => {
                    return Err(
                        "reference result backing needs its actual admitted producer",
                    );
                }
            };
            let (position, mut path) =
                descriptor_input_origin_path(types, parameters, origin)?;
            if path.iter().any(|step| {
                !matches!(
                    step,
                    ReferenceOriginProjection::Element
                        | ReferenceOriginProjection::Exact(
                            StructuralPathSegment::Field(_)
                                | StructuralPathSegment::Case(_)
                                | StructuralPathSegment::FixedIndex(_)
                                | StructuralPathSegment::Referent
                        )
                )
            }) {
                return Err("reference result needs a static captured input path");
            }
            let parameter = parameters
                .get(position)
                .ok_or("reference result input is unavailable")?;
            let endpoint = super::resolve_reference_origin_type(
                types.iter(),
                parameter.structural_type,
                &path,
            )?;
            if endpoint != referent {
                let Some(StructuralTypeShape::Reference {
                    referent: stored,
                    access: ceiling,
                }) = types
                    .iter()
                    .find(|row| row.id == endpoint)
                    .map(|row| &row.shape)
                else {
                    return Err("reference result changes its original referent");
                };
                if *stored != referent || !access.admits_referent(*ceiling) {
                    return Err("reference result changes its input type or access");
                }
                path.push(ReferenceOriginProjection::Exact(
                    StructuralPathSegment::Referent,
                ));
            } else if !access.admits(parameter.access) {
                return Err("reference result exceeds its input storage access");
            }
            if parameter.access == StructuralAccess::Owned
                && !path.iter().any(|step| {
                    matches!(
                        step,
                        ReferenceOriginProjection::Exact(StructuralPathSegment::Referent)
                    )
                })
            {
                return Err(
                    "reference result escapes owned input storage rather than its carried backing",
                );
            }
            // Check the whole path's access, not only its endpoint.
            // A mutable result cannot cross a shared ancestor. Every
            // element of one array has the same access, so the first
            // element stands for an unknown one; an empty array has
            // no element and fails closed.
            if !input_path_accessible(types, parameter, &path, access) {
                return Err("reference result exceeds an input path's access");
            }
            let source = ReferenceOriginSource {
                place: parameter.place,
                path,
                extent,
                access,
            };
            sources.push(terminal_psi::StructuralReferenceOrigin::Parameter(source));
        }
        sources.sort();
        sources.dedup();
        let first = sources
            .first()
            .and_then(|source| source.parameter())
            .map(|source| source.place);
        if sources
            .iter()
            .filter_map(|source| source.parameter())
            .any(|source| Some(source.place) != first)
        {
            return Err("reference result has alternatives from different input roots");
        }
        mappings.push(terminal_psi::StructuralReferenceResultSource {
            path: leaf.path,
            sources,
        });
    }
    mappings.sort();
    Ok(mappings)
}

pub fn descriptor_input_origin_path(
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
    if let Some(stripped) = projection.stripped_origin_path() {
        path.extend(stripped);
        return Ok(());
    }
    for segment in &projection.path {
        match segment {
            StructuralPathSegment::FixedRange { .. } => break,
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

fn input_path_accessible(
    types: &[StructuralTypeDeclaration],
    parameter: &StructuralParameterDeclaration,
    path: &[ReferenceOriginProjection],
    requested: StructuralAccess,
) -> bool {
    // Element permissions depend on the array or view, independently of the ordinal.
    let access_path = path
        .iter()
        .map(|step| match step {
            ReferenceOriginProjection::Exact(segment) => segment.clone(),
            ReferenceOriginProjection::Element => StructuralPathSegment::FixedIndex(0),
        })
        .collect::<Vec<_>>();
    crate::reference_source_projection_accessible(
        types.iter(),
        parameter.structural_type,
        parameter.multiplicity,
        parameter.access,
        &access_path,
        requested,
    )
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
            DescriptorContentsStep::Address(segment) => exact.push(*segment),
            DescriptorContentsStep::Element { element, .. } => {
                let collection = descriptor_location_type(types, root, &exact)
                    .ok_or("returned view contents have no typed collection")?;
                if super::collection_element_type(types.iter(), collection)
                    != Some(*element)
                {
                    return Err(
                        "returned view contents disagree with the input element type",
                    );
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
    inline_view
        && allowed.iter().any(|allowed| {
            allowed.place == actual.place
                && allowed.access == actual.access
                && allowed.path == actual.path
        })
}
