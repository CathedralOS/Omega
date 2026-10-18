//! Bind requirement-owned returned regions to captured invocation contents.
//! The selector chooses execution; this contract only bounds each returned
//! descriptor's backing and supplies no implementation-private guarantees.
use super::*;
use crate::descriptor_lifetimes::descriptor_location;

pub fn public_invocation(
    operation: semantic_vocabulary::OperationId,
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    slot: &terminal_psi::TerminalDynamicSlot,
    actuals: &[terminal_psi::StructuralArgument],
    result: Option<&terminal_psi::StructuralOperationResult>,
    place_types: &BTreeMap<PlaceId, StructuralTypeId>,
) -> Option<DescriptorInvocation> {
    if actuals.len() != slot.structural_parameters.len() {
        return None;
    }
    let mut arguments = Vec::with_capacity(actuals.len());
    let mut argument_contents = Vec::with_capacity(actuals.len());
    let mut invalidates_contents = false;
    for (actual, formal) in actuals.iter().zip(&slot.structural_parameters) {
        if actual.access != formal.access {
            return None;
        }
        let root = *place_types.get(&actual.place)?;
        arguments.push(descriptor_location(
            types,
            actual.place,
            root,
            &actual.path,
            None,
        ));
        argument_contents.push(descriptor_contents_query(
            types,
            actual.place,
            root,
            &actual.path,
            None,
        ));
        invalidates_contents |= slot_kind.may_write_indirect_contents(types, root, actual);
    }
    let mut results = Vec::new();
    if let Some(result) = result {
        let terminal_psi::BoundaryMachineResult::Structural(expected) = &slot.result else {
            return None;
        };
        if result.structural_type != expected.structural_type
            || result.multiplicity != expected.multiplicity
            || result.qualifications != expected.qualifications
            || result.projected_qualifications != expected.projected_qualifications
        {
            return None;
        }
        results = public_result_slots(types, slot_kind, slot, result.place)?;
    } else if matches!(
        slot.result,
        terminal_psi::BoundaryMachineResult::Structural(_)
    ) {
        return None;
    }
    Some(DescriptorInvocation {
        operation,
        target: DescriptorInvocationTarget::Public {
            extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
            results,
            invalidates_contents,
        },
        arguments,
        argument_contents,
        result: result
            .map(|result| crate::descriptor_lifetimes::DescriptorLocation::whole(result.place)),
    })
}

/// Canonical relative descriptor paths and input-content origins of a public
/// requirement. `place` is a scratch root used only to authenticate typed paths;
/// neither it nor a private callee identity enters the resulting relative rows.
pub fn public_result_slots(
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    slot: &terminal_psi::TerminalDynamicSlot,
    place: PlaceId,
) -> Option<Vec<DescriptorResultSlot>> {
    let terminal_psi::BoundaryMachineResult::Structural(result) = &slot.result else {
        return Some(Vec::new());
    };
    let mut results = Vec::new();
    // The public result owns its descriptor leaves, not the contents of their
    // referents. Broader replay inventories retain those external slots as
    // unknown; they cannot manufacture extra public lifetime declarations.
    let leaves = crate::borrowed_result_leaves(
        types.iter(),
        result.structural_type,
        result.multiplicity,
        slot.result_reference_regions.len(),
    )
    .ok()?;
    for leaf in leaves {
        let selected_kind = matches!(
            (&leaf.kind, slot_kind),
            (
                crate::BorrowedResultLeafKind::View,
                DescriptorSlotKind::Views
            ) | (
                crate::BorrowedResultLeafKind::Reference(_),
                DescriptorSlotKind::References | DescriptorSlotKind::PlacedOrigins,
            )
        );
        if !selected_kind {
            continue;
        }
        let leaf = descriptor_location(types, place, result.structural_type, &leaf.path, None)?;
        let mut origins = Vec::new();
        for region in &slot.result_reference_regions {
            let destination =
                descriptor_location(types, place, result.structural_type, &region.path, None)?;
            if destination.path != leaf.path {
                continue;
            }
            let index = match region.origin {
                terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index } => {
                    argument_index
                }
                terminal_psi::TerminalDynamicReferenceOrigin::Invocation { carrier_type } => {
                    let mut projection = Projection::new(&region.source_path);
                    projection.forget_address();
                    origins.push(DescriptorStoredOrigin::Invocation {
                        carrier_type,
                        projection,
                    });
                    continue;
                }
            };
            let parameter = usize::try_from(index).ok()?;
            let formal = slot.structural_parameters.get(parameter)?;
            origins.push(public_input_origin(
                types,
                formal,
                parameter,
                place,
                &region.source_path,
            )?);
        }
        if origins.is_empty() {
            return None;
        }
        results.push(DescriptorResultSlot {
            path: leaf.path,
            origins,
        });
    }
    Some(results)
}

/// Resolve the supplying descriptor before applying geometry below it. Plain
/// borrowed storage is not descriptor contents; a stored pointer/view crossing
/// captures its actual incoming contents before any call can overwrite them.
fn public_input_origin(
    types: &[StructuralTypeDeclaration],
    formal: &terminal_psi::TerminalDynamicStructuralFormal,
    parameter: usize,
    scratch_place: PlaceId,
    source_path: &[StructuralPathSegment],
) -> Option<DescriptorStoredOrigin> {
    let path =
        crate::reference_source_storage_path(types.iter(), formal.structural_type, source_path);
    let mut supplying = None;
    for position in 0..=path.len() {
        let prefix = &path[..position];
        let shape =
            crate::runtime_structural_path_tip(types.iter(), formal.structural_type, prefix);
        let stored_view = !prefix.is_empty()
            && (shape
                .as_ref()
                .is_some_and(|row| is_descriptor(types.iter(), row.id))
                || crate::runtime_structural_field(types.iter(), formal.structural_type, prefix)
                    .and_then(|field| field.field_type.byte_sequence_carrier())
                    .is_some_and(|carrier| {
                        matches!(
                            carrier,
                            ByteSequenceCarrier::BorrowedView { .. }
                                | ByteSequenceCarrier::BoundedView { .. }
                        )
                    }));
        let pointer_crossing = shape
            .as_ref()
            .is_some_and(|row| matches!(row.shape, StructuralTypeShape::Reference { .. }))
            && path.get(position) == Some(&StructuralPathSegment::Referent);
        if stored_view || pointer_crossing {
            supplying = Some((position, position + usize::from(pointer_crossing)));
        }
    }
    let suffix = supplying.map_or(path, |(_, suffix)| &path[suffix..]);
    let mut projection = Projection::new(suffix);
    projection.forget_address();
    match supplying {
        Some((prefix, _)) => {
            let selected = descriptor_location(
                types,
                scratch_place,
                formal.structural_type,
                &path[..prefix],
                None,
            )?;
            Some(DescriptorStoredOrigin::IncomingContents {
                position: parameter,
                steps: selected
                    .path
                    .into_iter()
                    .map(DescriptorContentsStep::Address)
                    .collect(),
                projection,
            })
        }
        None => Some(DescriptorStoredOrigin::Parameter {
            position: parameter,
            projection,
        }),
    }
}

/// Borrowing a descriptor grants its backing access, not replacement of the
/// descriptor's own carrier. Only contained writable slots can be reseated.
pub fn writable_descriptor_slots(
    types: &[StructuralTypeDeclaration],
    place_types: &BTreeMap<PlaceId, semantic_vocabulary::StructuralTypeId>,
    actual: &terminal_psi::StructuralArgument,
    slot_kind: DescriptorSlotKind,
) -> Option<Vec<DescriptorLocation>> {
    let ty = *place_types.get(&actual.place)?;
    let selected = descriptor_location(types, actual.place, ty, &actual.path, None);
    Some(
        slot_kind
            .slots(types, actual.place, ty)?
            .into_iter()
            .filter(|slot| {
                !slot.path.is_empty()
                    && selected
                        .as_ref()
                        .is_none_or(|selected| selected.contains(slot))
            })
            .collect(),
    )
}
