use super::*;
use crate::descriptors::descriptor_lifetimes::descriptor_location;
use terminal_psi::BoundaryMachineResult;
use terminal_psi::TerminalDynamicReferenceOrigin;

pub struct DescriptorMachineSignature<'a> {
    pub machine: MachineId,
    pub structural_parameters: &'a [terminal_psi::StructuralParameterDeclaration],
    pub result: Option<&'a terminal_psi::StructuralResultDeclaration>,
}

pub(crate) fn machine_invocation(
    owner: DescriptorInvocationOwner,
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    callee: &TerminalMachine,
    actuals: &[StructuralArgument],
    result: Option<&terminal_psi::StructuralOperationResult>,
    actual_root_types: impl IntoIterator<Item = Option<StructuralTypeId>>,
) -> Option<DescriptorInvocation> {
    machine_invocation_for_signature(
        owner,
        types,
        slot_kind,
        DescriptorMachineSignature {
            machine: callee.id,
            structural_parameters: &callee.structural_parameters,
            result: callee.result.structural(),
        },
        actuals,
        result,
        actual_root_types,
    )
}

pub fn machine_invocation_for_signature(
    owner: DescriptorInvocationOwner,
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    callee: DescriptorMachineSignature<'_>,
    actuals: &[StructuralArgument],
    result: Option<&terminal_psi::StructuralOperationResult>,
    actual_root_types: impl IntoIterator<Item = Option<StructuralTypeId>>,
) -> Option<DescriptorInvocation> {
    if callee.structural_parameters.len() != actuals.len() {
        return None;
    }
    let mut arguments = Vec::with_capacity(actuals.len());
    let mut argument_contents = Vec::with_capacity(actuals.len());
    let mut actual_root_types = actual_root_types.into_iter();
    for (actual, formal) in actuals.iter().zip(callee.structural_parameters) {
        if actual.access != formal.access {
            return None;
        }
        let root = actual_root_types.next()??;
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
    }
    if actual_root_types.next().is_some() {
        return None;
    }
    let result = result.filter(|result| {
        slot_kind.tracks_returned_value(types, result.place, result.structural_type)
    });
    if let Some(result) = result {
        let expected = callee.result?;
        if result.structural_type != expected.structural_type
            || result.multiplicity != expected.multiplicity
            || result.qualifications != expected.qualifications
            || !expected
                .projected_qualifications
                .iter()
                .all(|row| result.projected_qualifications.contains(row))
        {
            return None;
        }
    }
    Some(DescriptorInvocation {
        scalar_arguments: Vec::new(),
        owner,
        target: DescriptorInvocationTarget::Machine(callee.machine),
        arguments,
        argument_contents,
        result: result.map(|result| DescriptorLocation::whole(result.place)),
    })
}

impl DescriptorMachine {
    // A call that never returns can still read or write its arguments.
    // Both call forms use the shared signature checks and retain the
    // complete parameter list and supplied arguments at the pre-call cut.
    pub(crate) fn direct_invocation(
        &self,
        module: &TerminalModule,
        callee: &TerminalMachine,
        slot_kind: DescriptorSlotKind,
        site: semantic_vocabulary::InvocationSite,
        inputs: terminal_psi::ModuleInvocationInputs<'_>,
        result: Option<&terminal_psi::StructuralOperationResult>,
        selector: impl Fn(ValueId) -> Option<ScalarTerm>,
    ) -> Option<DescriptorInvocation> {
        if callee.id != inputs.callee || callee.parameters.len() != inputs.arguments.len() {
            return None;
        }
        let owner = match site {
            semantic_vocabulary::InvocationSite::Operation(operation) => {
                DescriptorInvocationOwner::Operation(operation)
            }
            semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
                DescriptorInvocationOwner::InvokeNoNormal(edge)
            }
            _ => return None,
        };
        // A witness can retain only the checked header of a non-returning
        // callee. Its arguments are still observed at the terminal edge,
        // but there is no callee body from which to infer writes.
        let mut call = if matches!(owner, DescriptorInvocationOwner::InvokeNoNormal(_))
            && callee.blocks.is_empty()
        {
            let header = crate::requirement_reference_result(module, callee.id).ok()??;
            public_invocation_at(
                owner,
                &module.types.structural_types,
                slot_kind,
                &header,
                Vec::new(),
                inputs.structural_arguments,
                result,
                |place| self.place_types.get(&place).copied(),
            )?
        } else {
            machine_invocation(
                owner,
                &module.types.structural_types,
                slot_kind,
                callee,
                inputs.structural_arguments,
                result,
                inputs
                    .structural_arguments
                    .iter()
                    .map(|actual| self.place_types.get(&actual.place).copied()),
            )?
        };
        call.scalar_arguments = descriptor_scalar_arguments(
            callee
                .parameters
                .iter()
                .map(|formal| (formal.id, formal.scalar_type)),
            inputs.arguments,
            selector,
        );
        Some(call)
    }
}

pub fn public_invocation(
    operation: semantic_vocabulary::OperationId,
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    slot: &impl terminal_psi::ReferenceResultShape,
    realizations: Vec<MachineId>,
    actuals: &[terminal_psi::StructuralArgument],
    result: Option<&terminal_psi::StructuralOperationResult>,
    place_types: &BTreeMap<PlaceId, StructuralTypeId>,
) -> Option<DescriptorInvocation> {
    public_invocation_at(
        DescriptorInvocationOwner::Operation(operation),
        types,
        slot_kind,
        slot,
        realizations,
        actuals,
        result,
        |place| place_types.get(&place).copied(),
    )
}

fn public_invocation_at(
    owner: DescriptorInvocationOwner,
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    slot: &impl terminal_psi::ReferenceResultShape,
    realizations: Vec<MachineId>,
    actuals: &[StructuralArgument],
    result: Option<&terminal_psi::StructuralOperationResult>,
    place_type: impl Fn(PlaceId) -> Option<StructuralTypeId>,
) -> Option<DescriptorInvocation> {
    if actuals.len() != slot.structural_parameter_count() {
        return None;
    }
    let mut arguments = Vec::with_capacity(actuals.len());
    let mut argument_contents = Vec::with_capacity(actuals.len());
    let mut invalidates_contents = false;
    for (index, actual) in actuals.iter().enumerate() {
        let (_, access) = slot.structural_parameter(index)?;
        if actual.access != access {
            return None;
        }
        let root = place_type(actual.place)?;
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
        let declared_result = slot.result();
        let BoundaryMachineResult::Structural(expected) = declared_result.as_ref() else {
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
    } else if matches!(owner, DescriptorInvocationOwner::Operation(_))
        && matches!(slot.result().as_ref(), BoundaryMachineResult::Structural(_))
    {
        return None;
    }
    Some(DescriptorInvocation {
        scalar_arguments: Vec::new(),
        owner,
        target: DescriptorInvocationTarget::Public {
            effects_machine: None,
            realizations,
            extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
            results,
            contents: Vec::new(),
            invalidates_contents,
        },
        arguments,
        argument_contents,
        result: result.map(|result| {
            crate::descriptors::descriptor_lifetimes::DescriptorLocation::whole(result.place)
        }),
    })
}

pub(crate) fn public_result_slots(
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    slot: &impl terminal_psi::ReferenceResultShape,
    place: PlaceId,
) -> Option<Vec<DescriptorResultSlot>> {
    let declared_result = slot.result();
    let BoundaryMachineResult::Structural(result) = declared_result.as_ref() else {
        return Some(Vec::new());
    };
    let mut results = Vec::new();
    let leaves = crate::borrowed_result_leaves(
        types.iter(),
        result.structural_type,
        result.multiplicity,
        slot.maximum_reference_leaves(),
    )
    .ok()?;
    for leaf in leaves {
        let selected_kind = matches!(
            (&leaf.kind, slot_kind),
            (
                crate::BorrowedResultLeafKind::View,
                DescriptorSlotKind::Views | DescriptorSlotKind::BorrowedStorage
            ) | (
                crate::BorrowedResultLeafKind::Reference(_),
                DescriptorSlotKind::References
                    | DescriptorSlotKind::BorrowedStorage
                    | DescriptorSlotKind::AccessOrigins,
            )
        );
        if !selected_kind {
            continue;
        }
        let leaf_kind = leaf.kind;
        let path = leaf.path;
        let tip_is_shared_byte_view = matches!(leaf_kind, crate::BorrowedResultLeafKind::View)
            && crate::runtime_structural_path_tip(types.iter(), result.structural_type, &path)
                .is_some_and(|tip| {
                    matches!(
                        tip.shape,
                        terminal_psi::StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView {
                                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                            }
                        )
                    )
                });
        let leaf = descriptor_location(types, place, result.structural_type, &path, None)?;
        let mut origins = Vec::new();
        for region in slot.reference_regions(&path) {
            let region = region.ok()?;
            let destination =
                descriptor_location(types, place, result.structural_type, &region.path, None)?;
            if destination.path != leaf.path {
                continue;
            }
            let index = match region.origin {
                TerminalDynamicReferenceOrigin::Parameter { argument_index } => argument_index,
                TerminalDynamicReferenceOrigin::Invocation { carrier_type } => {
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
            let (formal, _) = slot.structural_parameter(parameter)?;
            origins.push(public_input_origin(
                types,
                formal,
                parameter,
                place,
                &region.source_path,
            )?);
        }
        if origins.is_empty() {
            let no_borrowed_inputs = (0..slot.structural_parameter_count()).all(|index| {
                slot.structural_parameter(index)
                    .is_some_and(|(formal, access)| {
                        access == terminal_psi::StructuralAccess::Owned
                            && crate::borrowed_result_leaves(
                                types.iter(),
                                formal,
                                terminal_psi::StructuralMultiplicity::Unrestricted,
                                0,
                            )
                            .is_ok()
                    })
            });
            if tip_is_shared_byte_view && no_borrowed_inputs {
                origins.push(DescriptorStoredOrigin::Empty);
            } else {
                return None;
            }
        }
        results.push(DescriptorResultSlot {
            path: leaf.path,
            origins,
        });
    }
    Some(results)
}

fn public_input_origin(
    types: &[StructuralTypeDeclaration],
    formal: StructuralTypeId,
    parameter: usize,
    scratch_place: PlaceId,
    source_path: &[StructuralPathSegment],
) -> Option<DescriptorStoredOrigin> {
    let path = crate::reference_source_storage_path(types.iter(), formal, source_path);
    let mut supplying = None;
    for position in 0..=path.len() {
        let prefix = &path[..position];
        let shape = crate::runtime_structural_path_tip(types.iter(), formal, prefix);
        let stored_view = !prefix.is_empty()
            && (shape
                .as_ref()
                .is_some_and(|row| is_descriptor(types.iter(), row.id))
                || crate::runtime_structural_field(types.iter(), formal, prefix)
                    .and_then(|field| field.field_type.byte_sequence_carrier())
                    .is_some_and(|carrier| carrier.is_borrowed()));
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
            let selected =
                descriptor_location(types, scratch_place, formal, &path[..prefix], None)?;
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

pub fn selected_stored_effects_row(
    catalog: &terminal_psi::TerminalDynamicDispatchCatalog,
    table: u32,
    interface: u32,
    slot: u32,
) -> Option<semantic_vocabulary::MachineId> {
    let selected = catalog.table(table)?;
    let declaration = catalog.interface(interface)?;
    let signature = declaration.slots.get(usize::try_from(slot).ok()?)?;
    if selected.id != table
        || selected.interface != interface
        || declaration.id != interface
        || signature.slot != slot
        || !signature.family_tuple.is_empty()
        || selected.rows.len() != declaration.slots.len()
    {
        return None;
    }
    catalog.row(table, slot)
}

pub fn public_realizations(
    catalog: &terminal_psi::TerminalDynamicDispatchCatalog,
    interface: u32,
    slot: u32,
) -> Option<Vec<MachineId>> {
    catalog
        .interface(interface)?
        .slots
        .get(usize::try_from(slot).ok()?)?;
    let mut realizations = Vec::new();
    for table in catalog
        .tables
        .iter()
        .filter(|table| table.interface == interface)
    {
        if let Some(machine) = *table.rows.get(usize::try_from(slot).ok()?)?
            && !realizations.contains(&machine)
        {
            realizations.push(machine);
        }
    }
    Some(realizations)
}
