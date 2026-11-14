/*
We reconstruct the input projections consumed by instruction selection across
activation storage, address joins, aggregate results, arrival parameters, byte
concatenations and owners, established views, literal compares, native code
layouts, owned block homes, primitive locals, records, references, scalar/structural
arrays and locals, storage origins, and zeroed structurals.
*/

pub(crate) mod activation_storage_input {
    use calling_conventions::ValueShape;
    use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind as K};
    use selected_instructions::LocalStorageSlotId;

    pub(crate) fn footprint(ordinary: ValueShape, bound: u64) -> Option<ValueShape> {
        let offset = ordinary.byte_size.checked_add(7)? & !7;
        Some(ValueShape {
            byte_size: offset
                .checked_add(8)?
                .checked_add(u32::try_from(bound).ok()?)?,
            alignment: ordinary.alignment.max(8),
            ..ordinary
        })
    }

    pub(crate) fn local_shape(
        source: &LegalizedScalarFunction,
        slot: LocalStorageSlotId,
        ordinary: ValueShape,
    ) -> Option<ValueShape> {
        let LocalStorageSlotId::Structural { operation, place } = slot else {
            return Some(ordinary);
        };
        let mut producers = source
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|row| row.operation == operation);
        let row = producers.next()?;
        if producers.next().is_some() {
            return None;
        }
        let K::EstablishActivationClaim {
            result,
            bound,
            backing_type,
            layout,
            ..
        } = &row.kind
        else {
            return Some(ordinary);
        };
        if result.place != place || layout.shape != ordinary {
            return None;
        }
        let types = &source.structural.as_ref()?.structural_types;
        let backing = terminal_psi::find_structural_type(types, *backing_type)?;
        if backing.owns_linear_claim
            || backing.descriptor_cleanup
            || !backing.no_code_affine_cleanup
            || backing.shape
                != terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: *bound },
                )
        {
            return None;
        }
        footprint(ordinary, *bound)
    }
}

pub(crate) mod address_join_input {
    use crate::selection::place_declaration::{arrivals, declares_block_parameter};
    use crate::structural_inputs::structural_reference_input::{
        PointerSlot, owner_loan_geometry, projected_scalar_codec, reference_source_geometry,
        scalar_storage_type,
    };
    use abstract_operations::control_flow::address_joins::is_address_join_in;

    use calling_conventions::ValueShape;
    use legalized_operations::LegalizedScalarFunction;
    use selected_instructions::{
        LocalStorageSlotId, SelectedAddressBase, SelectedLocalStorageSlot,
        SelectedStructuralTransport, VirtualRegisterId,
    };
    use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
    use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};

    pub(crate) fn join(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<(BlockId, &StructuralParameterDeclaration)> {
        let signature = source.structural.as_ref()?;
        let mut matching = arrivals(source, place);
        let (block, parameter) = matching.next()?;
        (matching.next().is_none()
            && block != source.entry_block
            && is_address_join_in(parameter, &signature.structural_types)
            && declares_block_parameter(signature, place, block, parameter.position))
        .then_some((block, parameter))
    }

    pub(crate) fn carrier_shape(
        source: &LegalizedScalarFunction,
        parameter: &StructuralParameterDeclaration,
    ) -> ValueShape {
        let scalar = source.structural.as_ref().and_then(|signature| {
            scalar_storage_type(parameter.structural_type, &signature.structural_types)
        });
        ValueShape::integer(if scalar.is_some() { 16 } else { 8 }, 8)
    }

    pub(crate) fn projection(
        source: &LegalizedScalarFunction,
        target: BlockId,
        parameter: &StructuralParameterDeclaration,
        argument: &terminal_psi::StructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
        pointers: &[(PlaceId, VirtualRegisterId)],
        local_slots: &[SelectedLocalStorageSlot],
        element_views: &[crate::selection::element_view_homes::ElementViewHomes],
        byte_views: &[crate::selection::byte_view_homes::ByteViewHomes],
    ) -> Option<crate::selection::aggregate_result_input::BlockParameterProjection> {
        let signature = source.structural.as_ref()?;
        if argument.access != parameter.access
            || !abstract_operations::control_flow::address_joins::is_address_projection(
                &argument.path,
            )
            || join(source, parameter.place)?.0 != target
        {
            return None;
        }
        let owner_home = || {
            let (_, result) =
                crate::selection::aggregate_result_input::home(source, argument.place)?;
            (argument.path.is_empty()
                && abstract_operations::control_flow::owner_loans::is_bounded_owner_loan_in(
                    parameter,
                    &signature.structural_types,
                )
                && result.structural_type == parameter.structural_type
                && result.claims.is_empty()
                && result.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                && result.projected_qualifications.is_empty())
            .then_some(result.structural_type)
        };
        let root = root_type(source, argument.place, argument.access, &argument.path)
            .or_else(owner_home)?;
        let supplied = crate::selection::reference_input::source_access(source, argument.place)?;
        if supplied.structural_type != root
            || !terminal_semantics::reference_source_projection_accessible(
                signature.structural_types.iter(),
                root,
                supplied.multiplicity,
                supplied.access,
                &argument.path,
                argument.access,
            )
        {
            return None;
        }
        if abstract_operations::control_flow::address_joins::borrowed_referent_type(
            signature.structural_types.iter(),
            root,
            &argument.path,
            argument.access,
        ) != Some(parameter.structural_type)
        {
            return None;
        }
        let owner_loan = abstract_operations::control_flow::owner_loans::is_bounded_owner_loan_in(
            parameter,
            &signature.structural_types,
        );
        let mut geometry = if owner_loan {
            owner_loan_geometry(root, &argument.path, &signature.structural_types)
        } else {
            reference_source_geometry(root, &argument.path, &signature.structural_types)
        }?;
        if !crate::structural_inputs::runtime_indices_match(indices, &geometry.elements) {
            return None;
        }
        let referent = geometry.selected;
        let retained_scalar_descriptor = geometry
            .pointer_slots
            .last()
            .is_some_and(|slot| slot.scalar_descriptor);
        let byte_offset = if retained_scalar_descriptor {
            if argument.path.last() != Some(&terminal_psi::StructuralPathSegment::Referent) {
                return None;
            }
            geometry.pointer_slots.pop()?.byte_offset
        } else {
            geometry.byte_offset
        };
        let lent = crate::structural_inputs::shape(referent, &signature.structural_types)?;
        if referent != parameter.structural_type {
            return None;
        }
        let scalar = scalar_storage_type(referent, &signature.structural_types);
        let descriptor = retained_scalar_descriptor
            || argument.path.is_empty()
                && crate::selection::reference_input::borrowed_scalar_source(
                    source,
                    argument.place,
                )
                .is_some();
        let codec = scalar.and_then(|_| {
            projected_scalar_codec(root, &argument.path, &signature.structural_types)
        });
        let storage_offset = geometry
            .pointer_slots
            .first()
            .map_or(byte_offset, |slot| slot.byte_offset);
        let storage_bytes = if !geometry.pointer_slots.is_empty() {
            8
        } else if descriptor {
            16
        } else if let Some(codec) = codec {
            u32::from(codec.byte_size())
        } else {
            lent.byte_size
        };
        let (base, base_byte_offset) = projection_base(
            argument.place,
            storage_offset,
            storage_bytes,
            &mut geometry.pointer_slots,
            pointers,
            local_slots,
            element_views,
            byte_views,
        )?;
        if scalar.is_some() {
            let source = if descriptor {
                selected_instructions::SelectedScalarReferenceSource::Descriptor
            } else {
                selected_instructions::SelectedScalarReferenceSource::Canonical {
                    codec: codec?.encode()?,
                }
            };
            return Some(
                crate::selection::aggregate_result_input::BlockParameterProjection {
                    transport: SelectedStructuralTransport::ScalarReference {
                        base,
                        byte_offset,
                        source,
                        destination: LocalStorageSlotId::StructuralBlockParameter {
                            block: target,
                            place: parameter.place,
                        },
                    },
                    pointer_slots: geometry.pointer_slots,
                    indices: indices.to_vec(),
                    base_byte_offset,
                },
            );
        }
        Some(
            crate::selection::aggregate_result_input::BlockParameterProjection {
                transport: SelectedStructuralTransport::Address {
                    base,
                    byte_offset,
                    byte_count: lent.byte_size,
                    destination: LocalStorageSlotId::StructuralBlockParameter {
                        block: target,
                        place: parameter.place,
                    },
                },
                pointer_slots: geometry.pointer_slots,
                indices: indices.to_vec(),
                base_byte_offset,
            },
        )
    }

    pub(crate) fn projection_base(
        place: PlaceId,
        byte_offset: u32,
        byte_count: u32,
        pointer_slots: &mut Vec<PointerSlot>,
        pointers: &[(PlaceId, VirtualRegisterId)],
        local_slots: &[SelectedLocalStorageSlot],
        element_views: &[crate::selection::element_view_homes::ElementViewHomes],
        byte_views: &[crate::selection::byte_view_homes::ByteViewHomes],
    ) -> Option<(SelectedAddressBase, Option<VirtualRegisterId>)> {
        let view = element_views
            .iter()
            .find(|view| view.place == place)
            .map(|view| (view.backing_pointer, view.byte_offset))
            .or_else(|| {
                byte_views
                    .iter()
                    .find(|view| view.place == place)
                    .map(|view| (view.backing_pointer, view.byte_offset))
            });
        if !pointer_slots.is_empty()
            && let Some((pointer, offset)) = view
        {
            let first = pointer_slots.first()?;
            if first.byte_offset != 0 || first.indices != (0..0) || first.scalar_descriptor {
                return None;
            }
            pointer_slots.remove(0);
            return Some((SelectedAddressBase::Register(pointer), Some(offset)));
        }
        storage_base(place, byte_offset, byte_count, pointers, local_slots).map(|base| (base, None))
    }

    pub(crate) fn storage_base(
        place: PlaceId,
        byte_offset: u32,
        byte_count: u32,
        pointers: &[(PlaceId, VirtualRegisterId)],
        local_slots: &[SelectedLocalStorageSlot],
    ) -> Option<SelectedAddressBase> {
        if let Some((_, pointer)) = pointers.iter().find(|(candidate, _)| *candidate == place) {
            return Some(SelectedAddressBase::Register(*pointer));
        }
        let mut homes = local_slots.iter().filter(|slot| {
            slot.id.structural_place() == Some(place)
                && !matches!(slot.id, LocalStorageSlotId::StructuralBlockParameter { .. })
        });
        let home = homes.next()?;
        if homes.next().is_some()
            || byte_offset
                .checked_add(byte_count)
                .is_none_or(|end| end > home.byte_size)
        {
            return None;
        }
        Some(SelectedAddressBase::Local(home.id))
    }

    pub(crate) fn root_type(
        source: &LegalizedScalarFunction,
        place: PlaceId,
        access: StructuralAccess,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        if matches!(access, terminal_psi::exclusive_borrow_pair_pattern!()) {
            let permits = |parameter: &StructuralParameterDeclaration| {
                abstract_operations::control_flow::address_joins::loan_access_permitted(
                    source
                        .structural
                        .as_ref()
                        .into_iter()
                        .flat_map(|signature| signature.structural_types.iter()),
                    parameter.structural_type,
                    path,
                    parameter.access,
                    access,
                )
            };
            if let Some((_, result)) = crate::selection::aggregate_result_input::home(source, place)
            {
                return (matches!(
                    result.multiplicity,
                    terminal_psi::StructuralMultiplicity::Affine
                        | terminal_psi::StructuralMultiplicity::Unrestricted
                ))
                .then_some(result.structural_type);
            }
            if let Some((_, parameter)) = join(source, place) {
                return permits(parameter).then_some(parameter.structural_type);
            }
            if let Some((_, parameter)) =
                crate::selection::aggregate_result_input::block_home(source, place)
            {
                return permits(parameter).then_some(parameter.structural_type);
            }
            return source
                .structural
                .as_ref()?
                .parameters
                .iter()
                .find(|parameter| parameter.semantic.place == place)
                .filter(|parameter| permits(&parameter.semantic))
                .map(|parameter| parameter.semantic.structural_type);
        }
        if let Some(result) = source.instructions().find_map(|row| match &row.kind {
            legalized_operations::LegalizedScalarInstructionKind::EstablishElementView {
                result,
                ..
            }
            | legalized_operations::LegalizedScalarInstructionKind::ElementViewSubslice {
                result,
                ..
            } => (result.place == place).then_some(result),
            _ => None,
        }) {
            return Some(result.structural_type);
        }
        if let Some((_, result)) = crate::selection::aggregate_result_input::home(source, place) {
            return Some(result.structural_type);
        }
        if let Some((_, result, _, _)) =
            crate::selection::primitive_local_input::local(source, place)
        {
            return Some(result.structural_type);
        }
        if let Some((_, parameter)) =
            crate::selection::aggregate_result_input::block_home(source, place)
        {
            return Some(parameter.structural_type);
        }
        if let Some((_, parameter)) = join(source, place) {
            return (parameter.access != StructuralAccess::WriteOnlyBorrow)
                .then_some(parameter.structural_type);
        }
        let signature = source.structural.as_ref()?;
        signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == place)
            .filter(|parameter| parameter.semantic.access != StructuralAccess::WriteOnlyBorrow)
            .map(|parameter| parameter.semantic.structural_type)
    }

    pub(crate) fn loan_declaration(
        source: &LegalizedScalarFunction,
        place: PlaceId,
        access: StructuralAccess,
    ) -> Option<&StructuralParameterDeclaration> {
        source
            .structural
            .as_ref()?
            .parameters
            .iter()
            .map(|parameter| &parameter.semantic)
            .find(|parameter| parameter.place == place)
            .or_else(|| {
                (matches!(access, terminal_psi::exclusive_borrow_pair_pattern!()))
                    .then(|| join(source, place))
                    .flatten()
                    .map(|(_, declared)| declared)
            })
    }
}

pub(crate) mod aggregate_result_input {
    use crate::selection::place_declaration::{arrivals, declares_block_parameter};
    use crate::structural_inputs::structural_reference_input::{
        PointerSlot, ReferenceSourceGeometry, descriptor_source_geometry, leaf_copy_projection,
        owned_storage_shape, primitive_array_shape, reference_source_geometry, stored_value_shape,
    };

    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use semantic_vocabulary::PlaceId;
    use terminal_psi::{StructuralAccess, StructuralTypeShape, structural_case_position};

    pub(crate) fn owned_parameter_uses_local_home(
        source: &LegalizedScalarFunction,
        declaration: &terminal_psi::StructuralParameterDeclaration,
    ) -> Option<bool> {
        let mut parameters = source
            .structural
            .as_ref()?
            .parameters
            .iter()
            .filter(|parameter| parameter.semantic.place == declaration.place);
        let parameter = parameters.next()?;
        if parameters.next().is_some()
            || &parameter.semantic != declaration
            || declaration.access != StructuralAccess::Owned
        {
            return None;
        }
        Some(
            parameter.target.shape.byte_size != 0
                && parameter_home_required(source, declaration.place),
        )
    }

    pub(crate) fn parameter_home_required(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> bool {
        let Some(signature) = &source.structural else {
            return false;
        };
        if !signature.parameters.iter().any(|parameter| {
            parameter.semantic.place == place
                && parameter.semantic.access == StructuralAccess::Owned
                && signature.structural_types.iter().any(|declaration| {
                    declaration.id == parameter.semantic.structural_type
                        && matches!(
                            declaration.shape,
                            StructuralTypeShape::Reference { .. }
                                | StructuralTypeShape::Record { .. }
                                | terminal_psi::sum_or_mixed_shape_pattern!()
                                | StructuralTypeShape::FixedArray { .. }
                                | StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                )
                        )
                })
        }) {
            return false;
        }
        let binds = |successor: &legalized_operations::LegalizedScalarSuccessor| {
            successor
                .structural_bindings
                .iter()
                .any(|binding| binding.semantic.argument.place == place)
        };
        let forwarded = source.blocks.iter().any(|block| match &block.terminator {
            legalized_operations::LegalizedScalarTerminator::Jump { successor, .. } => {
                binds(successor)
            }
            legalized_operations::LegalizedScalarTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => binds(when_true) || binds(when_false),
            _ => false,
        });
        let dispatched = source.blocks.iter().any(|block| {
            matches!(
                &block.terminator,
                legalized_operations::LegalizedScalarTerminator::StructuralCase {
                    source: legalized_operations::LegalizedStructuralCaseSource::Parameter { declaration },
                    ..
                } if declaration.place == place
            )
        });
        let cleanup_borrow = source.blocks.iter().any(|block| block.terminator.cleanup_calls().any(|(_, call)|
            call.arguments.iter().any(|argument|
                matches!(argument, legalized_operations::LegalizedScalarArgument::Structural { semantic, target, .. }
                    if semantic.place == place && argument_transport_requires_home(target)))));
        let borrows_parameter = |call: &legalized_operations::LegalizedScalarCall| {
            call.arguments.iter().any(|argument| {
                matches!(argument, legalized_operations::LegalizedScalarArgument::Structural { semantic, .. }
                    if semantic.place == place && semantic.access != StructuralAccess::Owned)
            })
        };
        let terminal_borrow = source.blocks.iter().any(|block| {
            matches!(&block.terminator,
            legalized_operations::LegalizedScalarTerminator::InvokeNoNormal { call, .. }
                if borrows_parameter(call))
        });
        forwarded
            || dispatched
            || cleanup_borrow
            || terminal_borrow
            || source.instructions().any(|row| match &row.kind {
                LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, .. }
                | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                    source,
                    ..
                }
                | LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
                    source, ..
                }
                | LegalizedScalarInstructionKind::ElementViewLength { source, .. }
                | LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
                | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. }
                | LegalizedScalarInstructionKind::ElementViewSubslice { source, .. } => {
                    source.place == place
                }
                LegalizedScalarInstructionKind::StructuralCaseMembership {
                    source, path, ..
                } => *source == place && !path.is_empty(),
                LegalizedScalarInstructionKind::ActivateTaskRegister {
                    gdt,
                    task_state,
                    scratch,
                    ..
                } => [gdt, task_state, scratch]
                    .into_iter()
                    .any(|argument| argument.place == place),
                LegalizedScalarInstructionKind::ReloadSegmentDescriptors {
                    gdt, scratch, ..
                } => [gdt, scratch]
                    .into_iter()
                    .any(|argument| argument.place == place),
                LegalizedScalarInstructionKind::StorageAddress { source, .. }
                | LegalizedScalarInstructionKind::GlobalDescriptorTable { source, .. }
                | LegalizedScalarInstructionKind::PrimitiveScalarRead { source, .. }
                | LegalizedScalarInstructionKind::StructuralLeafCopy { source, .. }
                | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. } => {
                    *source == place
                }
                LegalizedScalarInstructionKind::EstablishStructuralArray { elements, .. } => {
                    elements
                        .iter()
                        .any(|argument| argument.place == place && !argument.path.is_empty())
                }
                LegalizedScalarInstructionKind::EstablishRecord { fields, .. }
                | LegalizedScalarInstructionKind::EstablishCase { fields, .. } => {
                    fields.iter().any(|field| {
                        matches!(&field.value, terminal_psi::RecordFieldValue::Structural(argument)
                        if argument.place == place && !argument.path.is_empty())
                    })
                }
                LegalizedScalarInstructionKind::EstablishReference { source, .. }
                | LegalizedScalarInstructionKind::EstablishAccessReference { source, .. }
                | LegalizedScalarInstructionKind::EstablishElementView { source, .. } => {
                    source.place == place
                }
                LegalizedScalarInstructionKind::AtomicAccess { location, .. }
                | LegalizedScalarInstructionKind::AccessorScalarAccess { location, .. } => {
                    location.place == place
                }
                LegalizedScalarInstructionKind::StaticInterrupt { token, .. } => {
                    token.place == place
                }
                LegalizedScalarInstructionKind::InstallStaticInterrupt { arguments, .. } => {
                    arguments.iter().any(|argument| argument.place == place)
                }
                LegalizedScalarInstructionKind::ByteSequenceStore {
                    destination,
                    source,
                    ..
                } => *destination == place || *source == place,
                LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
                    destination,
                    source,
                    ..
                } => destination.place == place || *source == place,
                LegalizedScalarInstructionKind::StoreStructuralField { destination, .. } => {
                    destination.place() == place
                }
                LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                    destination, ..
                }
                | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { destination, .. } => {
                    destination.place() == place
                }
                LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
                    left,
                    right,
                    ..
                } => left.source() == place || right.source() == place,
                LegalizedScalarInstructionKind::ByteSequenceEqual { left, right } => {
                    left.source() == place || right.source() == place
                }
                LegalizedScalarInstructionKind::ByteSequencePredicate { operand, .. } => {
                    operand.source() == place
                }
                LegalizedScalarInstructionKind::Call(call) => borrows_parameter(call),
                LegalizedScalarInstructionKind::NormalizedForeignCall(call) => {
                    call.structural_arguments.iter().any(|argument| {
                        argument.place == place && argument_transport_requires_home(argument)
                    })
                }
                _ => false,
            })
    }

    fn argument_transport_requires_home(
        argument: &target_operations::TargetStructuralArgument,
    ) -> bool {
        if argument.access != StructuralAccess::Owned {
            return true;
        }
        let target_operations::TargetStructuralArgumentSource::Placement(source) = &argument.source
        else {
            return false;
        };
        if !source.locations.iter().all(|location| {
            matches!(
                location,
                calling_conventions::ValueLocation::Register { .. }
            )
        }) {
            return false;
        }
        let can_forward = |offset: u32, size: u32| {
            let Some(offset) = argument.source_byte_offset.checked_add(offset) else {
                return false;
            };
            source.locations.iter().any(|location| matches!(
                location,
                calling_conventions::ValueLocation::Register { value_byte_offset, byte_size, .. }
                    if *value_byte_offset == offset && *byte_size >= size
            ))
        };
        if indirect_argument(&argument.destination).is_some() {
            return (0..argument.shape.byte_size)
                .step_by(8)
                .any(|offset| !can_forward(offset, (argument.shape.byte_size - offset).min(8)));
        }
        argument
            .destination
            .locations
            .iter()
            .any(|location| match location {
                calling_conventions::ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                }
                | calling_conventions::ValueLocation::Stack {
                    value_byte_offset,
                    byte_size,
                    ..
                } => !can_forward(*value_byte_offset, *byte_size),
                _ => true,
            })
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn membership_layout(
        source: &LegalizedScalarFunction,
        place: semantic_vocabulary::PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        case: semantic_vocabulary::StructuralCaseId,
    ) -> Option<(u32, u32, ReferenceSourceGeometry)> {
        let signature = source.structural.as_ref()?;
        let parameter = signature
            .parameters
            .iter()
            .map(|parameter| &parameter.semantic)
            .chain(source.block_structural_parameters())
            .find(|parameter| parameter.place == place);
        let identity = if let Some(parameter) = parameter {
            if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return None;
            }
            parameter.structural_type
        } else {
            let (_, result) = operation_result(source, place)?;
            result.structural_type
        };
        let geometry = reference_source_geometry(identity, path, &signature.structural_types)?;
        if !geometry.readable
            || geometry
                .pointer_slots
                .iter()
                .any(|slot| slot.scalar_descriptor)
        {
            return None;
        }
        let byte_offset = geometry.byte_offset;
        let declaration = signature.structural_type(geometry.selected)?;
        let (common, cases) = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                (fields.as_slice(), cases)
            }
            _ => return None,
        };
        let common = live_field_shapes(common.iter(), &signature.structural_types)?;
        let payloads = cases
            .iter()
            .map(|case| live_field_shapes(case.fields.iter(), &signature.structural_types))
            .collect::<Option<Vec<_>>>()?;
        let layout =
            calling_conventions::evaluate_conventional_sum_layout(&common, &payloads).ok()?;
        if layout.tag_byte_offset != 0
            || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
        {
            return None;
        }
        cases
            .iter()
            .position(|candidate| candidate.id == case)
            .and_then(|ordinal| u32::try_from(ordinal).ok())
            .map(|tag| (tag, byte_offset, geometry))
    }

    pub(crate) fn call_completion<'a>(
        source: &LegalizedScalarFunction,
        instruction: &'a LegalizedScalarInstruction,
    ) -> Option<(
        &'a terminal_psi::StructuralOperationResult,
        &'a calling_conventions::ValuePlacement,
        calling_conventions::CallingPolicy,
    )> {
        match &instruction.kind {
            LegalizedScalarInstructionKind::Call(call) => {
                let (result, placement) = call_result(source, call)?;
                Some((result, placement, call.call_plan.policy))
            }
            LegalizedScalarInstructionKind::NormalizedForeignCall(call) => {
                let target_operations::TargetBoundaryResult::Structural(home) = &call.result else {
                    return None;
                };
                let (producer, result) = home.operation_result()?;
                if producer != instruction.operation
                    || operation_result(source, result.place) != Some((producer, result))
                {
                    return None;
                }
                Some((
                    result,
                    call.binding.boundary_entry_plan.call.result.as_ref()?,
                    call.binding.boundary_entry_plan.call.policy,
                ))
            }
            _ => None,
        }
    }

    pub(crate) fn call_result<'a>(
        source: &LegalizedScalarFunction,
        call: &'a legalized_operations::LegalizedScalarCall,
    ) -> Option<(
        &'a terminal_psi::StructuralOperationResult,
        &'a calling_conventions::ValuePlacement,
    )> {
        let result = call.structural_result.as_ref()?;
        let declaration = terminal_psi::find_structural_type(
            &source.structural.as_ref()?.structural_types,
            result.structural_type,
        )?;
        let shape = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Record { .. }
            | terminal_psi::StructuralTypeShape::FixedArray { .. } => primitive_array_shape(
                result.structural_type,
                &source.structural.as_ref()?.structural_types,
            )
            .or_else(|| {
                crate::structural_inputs::structural_reference_input::shape(
                    result.structural_type,
                    &source.structural.as_ref()?.structural_types,
                )
            })?,
            terminal_psi::StructuralTypeShape::Reference { .. }
            | terminal_psi::StructuralTypeShape::Sum { .. }
            | terminal_psi::StructuralTypeShape::Mixed { .. } => {
                crate::structural_inputs::structural_reference_input::shape(
                    result.structural_type,
                    &source.structural.as_ref()?.structural_types,
                )?
            }
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            ) => owned_storage_shape(
                result.structural_type,
                &source.structural.as_ref()?.structural_types,
            )?,
            _ if terminal_semantics::descriptor_results::is_descriptor(
                source.structural.as_ref()?.structural_types.iter(),
                result.structural_type,
            ) && matches!(
                result.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            ) =>
            {
                crate::structural_inputs::structural_reference_input::shape(
                    result.structural_type,
                    &source.structural.as_ref()?.structural_types,
                )?
            }
            _ => return None,
        };
        let placement = call.result_placement.as_ref()?;
        if call.call_plan.result.as_ref() != Some(placement)
            || placement.shape != shape
            || !(direct_fragments(placement)
                || indirect_result(placement, call.call_plan.policy).is_some())
        {
            return None;
        }
        Some((result, placement))
    }

    pub(crate) fn returned_descriptor<'a>(
        source: &'a LegalizedScalarFunction,
        value: &legalized_operations::LegalizedScalarReturnValue,
    ) -> Option<(
        semantic_vocabulary::PlaceId,
        &'a calling_conventions::ValuePlacement,
        Option<selected_instructions::LocalStorageSlotId>,
    )> {
        let legalized_operations::LegalizedScalarReturnValue::SharedDescriptor { place } = value
        else {
            return None;
        };
        let signature = source.structural.as_ref()?;
        let declared = signature.result.as_ref()?;
        let identity = crate::selection::established_view_input::view_type(source, *place)
            .or_else(|| {
                crate::selection::established_view_input::element_view_type(source, *place)
            })?;
        let placement = source.call_plan.result.as_ref()?;
        if identity != declared.structural_type
            || !matches!(
                declared.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            )
            || !terminal_semantics::view_descriptor_projected_qualifications(
                signature.structural_types.iter(),
                declared.structural_type,
                &declared.projected_qualifications,
            )
            || !terminal_semantics::descriptor_results::is_descriptor(
                signature.structural_types.iter(),
                identity,
            )
            || placement.shape != calling_conventions::ValueShape::integer(16, 8)
            || !(direct_fragments(placement)
                || indirect_result(placement, source.call_plan.policy).is_some())
        {
            return None;
        }
        if signature
            .parameters
            .iter()
            .any(|parameter| parameter.semantic.place == *place)
        {
            return Some((*place, placement, None));
        }
        for block in &source.blocks {
            if block.structural_parameter(*place).is_some() {
                return Some((
                    *place,
                    placement,
                    Some(
                        selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                            block: block.id,
                            place: *place,
                        },
                    ),
                ));
            }
            for row in &block.instructions {
                if row
                    .kind
                    .get_structural_result()
                    .is_some_and(|result| result.place == *place)
                    || matches!(&row.kind,
                        LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                            destination, ..
                        } if destination.id == *place)
                {
                    return Some((
                        *place,
                        placement,
                        Some(selected_instructions::LocalStorageSlotId::Structural {
                            operation: row.operation,
                            place: *place,
                        }),
                    ));
                }
            }
        }
        None
    }

    pub(crate) fn returned_parameter<'a>(
        source: &'a LegalizedScalarFunction,
        value: &legalized_operations::LegalizedScalarReturnValue,
    ) -> Option<(
        &'a legalized_operations::LegalizedCallUnitParameter,
        &'a calling_conventions::ValuePlacement,
    )> {
        let legalized_operations::LegalizedScalarReturnValue::StructuralParameter { place } = value
        else {
            return None;
        };
        let signature = source.structural.as_ref()?;
        let declared = signature.result.as_ref()?;
        let parameter = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == *place)?;
        let semantic = &parameter.semantic;
        if semantic.access != terminal_psi::StructuralAccess::Owned
            || semantic.structural_type != declared.structural_type
            || semantic.multiplicity != declared.multiplicity
        {
            return None;
        }
        let placement = source.call_plan.result.as_ref()?;
        let shape =
            crate::structural_inputs::parameter_shape(semantic, &signature.structural_types)?;
        if parameter.target.shape != shape
            || parameter.target.placement.shape != shape
            || placement.shape != shape
            || !(direct_fragments(placement)
                || indirect_result(placement, source.call_plan.policy).is_some())
            || !(inline_argument_fragments(&parameter.target.placement)
                || crate::structural_inputs::structural_unit_input::owned_indirect_pointer(
                    semantic,
                    &parameter.target.placement,
                )
                .is_some())
        {
            return None;
        }
        Some((parameter, placement))
    }

    pub(crate) fn inline_argument_fragments(
        placement: &calling_conventions::ValuePlacement,
    ) -> bool {
        if !placement.shape.alignment.is_power_of_two() {
            return false;
        }
        if direct_fragments(placement) {
            return true;
        }
        if placement.shape.class != calling_conventions::ValueClass::Integer
            || placement.shape.byte_size == 0
        {
            return false;
        }
        let Some(calling_conventions::ValueLocation::Stack {
            stack_byte_offset: base,
            ..
        }) = placement.locations.first()
        else {
            return false;
        };
        let mut offset = 0u32;
        for location in &placement.locations {
            let calling_conventions::ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } = location
            else {
                return false;
            };
            if *value_byte_offset != offset
                || *byte_size == 0
                || !alignment.is_power_of_two()
                || !stack_byte_offset.is_multiple_of(u32::from(*alignment))
                || base.checked_add(offset) != Some(*stack_byte_offset)
            {
                return false;
            }
            let Some(next) = offset.checked_add(*byte_size) else {
                return false;
            };
            offset = next;
        }
        offset == placement.shape.byte_size
    }

    pub(crate) fn owned_argument_placement(
        placement: &calling_conventions::ValuePlacement,
    ) -> bool {
        if indirect_argument(placement).is_some()
            || crate::selection::scalar_call_abi::empty_aggregate_placement(placement)
        {
            return true;
        }
        if !placement.shape.alignment.is_power_of_two()
            || matches!(
                placement.shape.class,
                calling_conventions::ValueClass::BorrowedReference
                    | calling_conventions::ValueClass::BorrowedScalarReference
            )
            || placement.shape.byte_size == 0
        {
            return false;
        }
        let mut ranges = Vec::with_capacity(placement.locations.len());
        for location in &placement.locations {
            let (offset, byte_size) = match location {
                calling_conventions::ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                } if (1..=8).contains(byte_size) => (*value_byte_offset, *byte_size),
                calling_conventions::ValueLocation::Stack {
                    value_byte_offset,
                    byte_size,
                    stack_byte_offset,
                    alignment,
                } if alignment.is_power_of_two()
                    && stack_byte_offset.is_multiple_of(u32::from(*alignment))
                    && stack_byte_offset.checked_add(*byte_size).is_some() =>
                {
                    (*value_byte_offset, *byte_size)
                }
                _ => return false,
            };
            if byte_size == 0 {
                return false;
            }
            ranges.push((offset, byte_size));
        }
        ranges.sort_unstable_by_key(|(offset, _)| *offset);
        let mut end = 0_u32;
        for (offset, byte_size) in ranges {
            if offset != end {
                return false;
            }
            let Some(next) = end.checked_add(byte_size) else {
                return false;
            };
            end = next;
        }
        end == placement.shape.byte_size
    }

    pub(crate) fn indirect_argument(
        placement: &calling_conventions::ValuePlacement,
    ) -> Option<(calling_conventions::IndirectPointerLocation, u32)> {
        let [
            calling_conventions::ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: Some(copy_offset),
                byte_size,
                alignment,
            },
        ] = placement.locations.as_slice()
        else {
            return None;
        };
        if matches!(
            placement.shape.class,
            calling_conventions::ValueClass::BorrowedReference
                | calling_conventions::ValueClass::BorrowedScalarReference
        ) || *byte_size == 0
            || *byte_size != placement.shape.byte_size
            || *alignment != placement.shape.alignment
            || !alignment.is_power_of_two()
            || !copy_offset.is_multiple_of(u32::from((*alignment).clamp(8, 16)))
        {
            return None;
        }
        copy_offset
            .checked_add(*byte_size)?
            .checked_add(u32::from(alignment.saturating_sub(16)))?;
        Some((*pointer, *copy_offset))
    }

    pub(crate) fn direct_fragments(placement: &calling_conventions::ValuePlacement) -> bool {
        if crate::selection::scalar_call_abi::empty_aggregate_placement(placement) {
            return true;
        }
        if placement.shape.class != calling_conventions::ValueClass::Integer
            || !(1..=2).contains(&placement.locations.len())
        {
            return false;
        }
        let mut offset = 0;
        for location in &placement.locations {
            let calling_conventions::ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            } = location
            else {
                return false;
            };
            if *value_byte_offset != offset || !(1..=8).contains(byte_size) {
                return false;
            }
            let Some(next) = offset.checked_add(*byte_size) else {
                return false;
            };
            offset = next;
        }
        offset == placement.shape.byte_size
    }

    pub(crate) fn returned<'a>(
        source: &'a LegalizedScalarFunction,
        value: &legalized_operations::LegalizedScalarReturnValue,
    ) -> Option<(
        selected_instructions::LocalStorageSlotId,
        &'a calling_conventions::ValuePlacement,
    )> {
        let legalized_operations::LegalizedScalarReturnValue::Structural { source: owner } = value
        else {
            return None;
        };
        let declared = source.structural.as_ref()?.result.as_ref()?;
        let (slot, shape, structural_type, multiplicity) = match owner {
            legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                operation,
                result: returned,
            } => {
                let (producer, result) = home(source, returned.place)?;
                if producer != *operation {
                    return None;
                }
                let shape = crate::structural_inputs::structural_reference_input::shape(
                    result.structural_type,
                    &source.structural.as_ref()?.structural_types,
                )?;
                if result != returned {
                    return None;
                }
                (
                    selected_instructions::LocalStorageSlotId::Structural {
                        operation: *operation,
                        place: result.place,
                    },
                    shape,
                    result.structural_type,
                    result.multiplicity,
                )
            }
            legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                block,
                declaration,
            } => {
                let parameter = source
                    .block(*block)?
                    .structural_parameter(declaration.place)?;
                if parameter != declaration {
                    return None;
                }
                (
                    selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                        block: *block,
                        place: declaration.place,
                    },
                    block_parameter_shape(source, parameter)?,
                    parameter.structural_type,
                    parameter.multiplicity,
                )
            }
            legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
            | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter { .. } => {
                return None;
            }
        };
        if structural_type != declared.structural_type || multiplicity != declared.multiplicity {
            return None;
        }
        let placement = source.call_plan.result.as_ref()?;
        if placement.shape != shape
            || !(direct_fragments(placement)
                || indirect_result(placement, source.call_plan.policy).is_some())
        {
            return None;
        }
        Some((slot, placement))
    }

    pub(crate) fn fields(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
    ) -> Option<(
        usize,
        Vec<(
            u32,
            calling_conventions::ValueShape,
            Option<semantic_vocabulary::ScalarType>,
        )>,
    )> {
        let LegalizedScalarInstructionKind::EstablishCase {
            result,
            result_case,
            fields,
            layout,
        } = &row.kind
        else {
            return None;
        };
        if row.result.is_some()
            || operation_result(source, result.place) != Some((row.operation, result))
        {
            return None;
        }
        let signature = source.structural.as_ref()?;
        let mut declarations = signature
            .structural_types
            .iter()
            .filter(|declaration| declaration.id == result.structural_type);
        let declaration = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        let (common, cases) = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                (fields.as_slice(), cases)
            }
            _ => return None,
        };
        let common = common
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        let common_shapes = live_field_shapes(common.iter().copied(), &signature.structural_types)?;
        let ordinal = structural_case_position(cases, *result_case)?;
        if cases.iter().filter(|case| case.id == *result_case).count() != 1 {
            return None;
        }
        let payloads = cases
            .iter()
            .map(|case| live_field_shapes(case.fields.iter(), &signature.structural_types))
            .collect::<Option<Vec<_>>>()?;
        let payload = cases[ordinal]
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if calling_conventions::evaluate_conventional_sum_layout(&common_shapes, &payloads).ok()?
            != *layout
            || fields.len() != common.len() + payload.len()
        {
            return None;
        }
        fields
            .iter()
            .zip(common.iter().copied().chain(payload))
            .zip(
                layout
                    .common_fields
                    .iter()
                    .chain(&layout.cases[ordinal].fields),
            )
            .map(|((field, declaration), placed)| {
                let shape = crate::selection::record_input::field_shape(
                    source,
                    result,
                    field,
                    declaration,
                )?;
                (shape == placed.shape).then_some((
                    placed.byte_offset,
                    shape,
                    declaration.field_type.scalar_type(),
                ))
            })
            .collect::<Option<Vec<_>>>()
            .map(|fields| (ordinal, fields))
    }

    pub(crate) fn block_parameter_shape(
        source: &LegalizedScalarFunction,
        parameter: &terminal_psi::StructuralParameterDeclaration,
    ) -> Option<calling_conventions::ValueShape> {
        let signature = source.structural.as_ref()?;
        if matches!(
            parameter.access,
            terminal_psi::reading_borrow_pair_pattern!()
        ) {
            if !terminal_semantics::view_descriptor_parameter(
                parameter,
                signature.structural_types.iter(),
            ) {
                return None;
            }
            return crate::structural_inputs::structural_reference_input::shape(
                parameter.structural_type,
                &signature.structural_types,
            );
        }
        if parameter.access != terminal_psi::StructuralAccess::Owned {
            return None;
        }
        stored_value_shape(parameter.structural_type, &signature.structural_types)
    }

    pub(crate) struct BlockParameterProjection {
        pub(crate) transport: selected_instructions::SelectedStructuralTransport,
        pub(crate) pointer_slots: Vec<PointerSlot>,
        pub(crate) indices: Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
        pub(crate) base_byte_offset: Option<selected_instructions::VirtualRegisterId>,
    }

    pub(crate) fn block_parameter_transport(
        source: &LegalizedScalarFunction,
        target: semantic_vocabulary::BlockId,
        parameter: &terminal_psi::StructuralParameterDeclaration,
        argument: &terminal_psi::StructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
        pointers: &[(PlaceId, selected_instructions::VirtualRegisterId)],
        local_slots: &[selected_instructions::SelectedLocalStorageSlot],
        element_views: &[crate::selection::element_view_homes::ElementViewHomes],
        byte_views: &[crate::selection::byte_view_homes::ByteViewHomes],
    ) -> Option<BlockParameterProjection> {
        use selected_instructions::{LocalStorageSlotId, SelectedStructuralTransport};
        use terminal_semantics::BorrowedReferentPresentation;
        if argument.access != parameter.access
            || !(if argument.access == StructuralAccess::Owned {
                abstract_operations::control_flow::address_joins::is_static_projection(
                    &argument.path,
                )
            } else {
                abstract_operations::control_flow::address_joins::is_address_projection(
                    &argument.path,
                )
            })
        {
            return None;
        }
        let mut pointer_slots = Vec::new();
        let mut presented_type = None;
        let source_byte_offset = if argument.access == StructuralAccess::Owned {
            if !indices.is_empty() {
                return None;
            }
            let declarations = &source.structural.as_ref()?.structural_types;
            let root = crate::selection::address_join_input::root_type(
                source,
                argument.place,
                argument.access,
                &argument.path,
            )?;
            let (selected, offset) = leaf_copy_projection(
                root,
                &argument.path,
                parameter.structural_type,
                declarations,
            )?;
            if selected != parameter.structural_type
                && terminal_semantics::borrowed_referent_presentation(
                    declarations.iter(),
                    selected,
                    parameter.structural_type,
                ) != Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
            {
                return None;
            }
            let root_shape = stored_value_shape(root, declarations)?;
            let selected_shape = block_parameter_shape(source, parameter)?;
            if offset.checked_add(selected_shape.byte_size)? > root_shape.byte_size
                || offset % u32::from(selected_shape.alignment) != 0
            {
                return None;
            }
            offset
        } else {
            let declarations = &source.structural.as_ref()?.structural_types;
            let root = crate::selection::established_view_input::view_type(source, argument.place)
                .or_else(|| {
                    crate::selection::established_view_input::element_view_type(
                        source,
                        argument.place,
                    )
                })
                .or_else(|| crate::selection::reference_input::root_type(source, argument.place))?;
            let supplied =
                crate::selection::reference_input::source_access(source, argument.place)?;
            if supplied.structural_type != root
                || !terminal_semantics::descriptor_source_projection_accessible(
                    declarations.iter(),
                    root,
                    supplied.multiplicity,
                    supplied.access,
                    &argument.path,
                    argument.access,
                )
            {
                return None;
            }
            let geometry = descriptor_source_geometry(root, &argument.path, declarations)?;
            if !geometry.readable
                || !crate::structural_inputs::runtime_indices_match(indices, &geometry.elements)
                || geometry
                    .pointer_slots
                    .iter()
                    .any(|slot| slot.scalar_descriptor)
            {
                return None;
            }
            presented_type = Some(geometry.selected);
            pointer_slots = geometry.pointer_slots;
            geometry.byte_offset
        };
        let shape = block_parameter_shape(source, parameter)?;
        if shape.byte_size == 0 {
            return Some(BlockParameterProjection {
                transport: SelectedStructuralTransport::Unused,
                pointer_slots,
                indices: indices.to_vec(),
                base_byte_offset: None,
            });
        }
        let destination = LocalStorageSlotId::StructuralBlockParameter {
            block: target,
            place: parameter.place,
        };
        let presentation = if parameter.access == StructuralAccess::Owned {
            BorrowedReferentPresentation::Unchanged
        } else {
            let actual = presented_type?;
            terminal_semantics::borrowed_referent_presentation(
                source.structural.as_ref()?.structural_types.iter(),
                actual,
                parameter.structural_type,
            )?
        };
        let storage_offset = pointer_slots
            .first()
            .map_or(source_byte_offset, |slot| slot.byte_offset);
        let storage_bytes = if pointer_slots.is_empty() {
            match presentation {
                BorrowedReferentPresentation::Unchanged => shape.byte_size,
                BorrowedReferentPresentation::AddressOfBacking { byte_count } => {
                    u32::try_from(byte_count).ok()?
                }
            }
        } else {
            8
        };
        let (base, base_byte_offset) = crate::selection::address_join_input::projection_base(
            argument.place,
            storage_offset,
            storage_bytes,
            &mut pointer_slots,
            pointers,
            local_slots,
            element_views,
            byte_views,
        )?;
        let transport = match presentation {
            BorrowedReferentPresentation::Unchanged => {
                let selected_instructions::SelectedAddressBase::Register(pointer) = base else {
                    return None;
                };
                SelectedStructuralTransport::WholeValue {
                    argument: pointer,
                    source_byte_offset,
                    destination,
                    byte_size: shape.byte_size,
                    alignment: shape.alignment,
                }
            }
            BorrowedReferentPresentation::AddressOfBacking { byte_count } => {
                let byte_count = u32::try_from(byte_count).ok()?;
                SelectedStructuralTransport::Address {
                    base,
                    byte_offset: source_byte_offset,
                    byte_count,
                    destination,
                }
            }
        };
        Some(BlockParameterProjection {
            transport,
            pointer_slots,
            indices: indices.to_vec(),
            base_byte_offset,
        })
    }

    pub(crate) fn indirect_result(
        placement: &calling_conventions::ValuePlacement,
        policy: calling_conventions::CallingPolicy,
    ) -> Option<calling_conventions::MachineRegister> {
        use calling_conventions::{
            CallSignature, IndirectPointerLocation, ValueLocation, evaluate_call_plan,
        };
        if placement.shape.class != calling_conventions::ValueClass::Integer
            || placement.shape.byte_size == 0
            || !placement.shape.alignment.is_power_of_two()
        {
            return None;
        }
        let [
            ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: None,
                byte_size,
                alignment,
            },
        ] = placement.locations.as_slice()
        else {
            return None;
        };
        if *byte_size != placement.shape.byte_size || *alignment != placement.shape.alignment {
            return None;
        }
        let canonical = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(placement.shape),
            },
        )
        .ok()?;
        (canonical.result.as_ref() == Some(placement)).then_some(*register)
    }

    pub(crate) fn operation_result(
        source: &LegalizedScalarFunction,
        place: semantic_vocabulary::PlaceId,
    ) -> Option<(
        semantic_vocabulary::OperationId,
        &terminal_psi::StructuralOperationResult,
    )> {
        let mut selected = source.instructions().filter_map(|row| {
            let result = row.kind.get_structural_result()?;
            (result.place == place).then_some((row, result))
        });
        let (row, result) = selected.next()?;
        let operation = row.operation;
        let mut declarations = source
            .structural
            .as_ref()?
            .structural_places
            .iter()
            .filter(|declaration| declaration.id == place);
        let declaration = declarations.next()?;
        if selected.next().is_some()
            || declarations.next().is_some()
            || declaration.kind
                != (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: result.structural_type,
                })
        {
            return None;
        }
        Some((operation, result))
    }

    pub(crate) fn home(
        source: &LegalizedScalarFunction,
        place: semantic_vocabulary::PlaceId,
    ) -> Option<(
        semantic_vocabulary::OperationId,
        &terminal_psi::StructuralOperationResult,
    )> {
        let (operation, result) = operation_result(source, place)?;
        let signature = source.structural.as_ref()?;
        crate::structural_inputs::shape(result.structural_type, &signature.structural_types)?;
        Some((operation, result))
    }

    pub(crate) fn block_home(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<(
        semantic_vocabulary::BlockId,
        &terminal_psi::StructuralParameterDeclaration,
    )> {
        let mut matching = arrivals(source, place);
        let (block, parameter) = matching.next()?;
        let signature = source.structural.as_ref()?;
        if matching.next().is_some()
            || parameter.access != StructuralAccess::Owned
            || !declares_block_parameter(signature, place, block, parameter.position)
        {
            return None;
        }
        block_parameter_shape(source, parameter)?;
        Some((block, parameter))
    }

    pub(crate) fn owned_store_destination(
        source: &LegalizedScalarFunction,
        producer: semantic_vocabulary::OperationId,
        expected: &terminal_psi::StructuralOperationResult,
    ) -> bool {
        operation_result(source, expected.place) == Some((producer, expected))
    }

    fn live_field_shapes<'field>(
        fields: impl Iterator<Item = &'field terminal_psi::StructuralFieldDeclaration>,
        declarations: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<Vec<calling_conventions::ValueShape>> {
        fields
            .filter(|field| !field.relevance.is_erased())
            .map(|field| {
                crate::structural_inputs::structural_reference_input::field_shape(
                    &field.field_type,
                    declarations,
                    &mut Vec::new(),
                )
            })
            .collect()
    }
}

pub(crate) mod arrival_parameter_input {
    use legalized_operations::LegalizedScalarFunction;
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
    use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};

    pub(crate) fn store_destination(
        source: &LegalizedScalarFunction,
        expected: &StructuralParameterDeclaration,
    ) -> bool {
        let Some(signature) = source.structural.as_ref() else {
            return false;
        };
        if !matches!(
            expected.access,
            StructuralAccess::Owned | terminal_psi::exclusive_borrow_pair_pattern!()
        ) {
            return false;
        }
        parameter(source, expected.place) == Some(expected)
            && match signature
                .structural_place(expected.place)
                .map(|row| row.kind)
            {
                Some(StructuralPlaceKind::BlockParameter { block, .. }) => match expected.access {
                    StructuralAccess::Owned => {
                        crate::selection::owned_block_home_input::store_destination(
                            source, block, expected,
                        )
                    }
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => true,
                    _ => false,
                },
                _ => true,
            }
    }

    pub(crate) fn parameter(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<&StructuralParameterDeclaration> {
        let signature = source.structural.as_ref()?;
        let mut places = signature
            .structural_places
            .iter()
            .filter(|row| row.id == place);
        let declaration = places.next()?;
        if places.next().is_some() {
            return None;
        }
        let mut parameters = signature
            .parameters
            .iter()
            .map(|row| &row.semantic)
            .chain(source.block_structural_parameters())
            .filter(|parameter| parameter.place == place);
        let parameter = parameters.next()?;
        if parameters.next().is_some() {
            return None;
        }
        let matches = match declaration.kind {
            StructuralPlaceKind::Parameter { position } => {
                position == parameter.position
                    && signature
                        .parameters
                        .get(position as usize)
                        .is_some_and(|row| row.semantic == *parameter)
            }
            StructuralPlaceKind::BlockParameter { block, position } => {
                block != source.entry_block
                    && position == parameter.position
                    && source
                        .block(block)
                        .and_then(|row| row.structural_parameters.get(position as usize))
                        == Some(parameter)
            }
            _ => false,
        };
        matches.then_some(parameter)
    }
}

pub(crate) mod byte_concatenation_input {
    use crate::selection::aggregate_result_input::call_result;
    use crate::selection::place_declaration::declares_operation_result;

    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::control_flow::ByteSequenceOperand;
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralTypeShape,
    };

    pub(crate) fn owned_type(
        function: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        let signature = function.structural.as_ref()?;
        let mut matching = function.instructions().filter_map(|row| {
            let result = match &row.kind {
                LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
                    result,
                    ..
                } => result,
                LegalizedScalarInstructionKind::Call(call) => call_result(function, call)?.0,
                _ => return None,
            };
            (result.place == place
                && result.multiplicity == StructuralMultiplicity::Unrestricted
                && result.projected_qualifications.is_empty()
                && result.qualification_establishments.is_empty()
                && result.claims.is_empty()
                && signature.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && declaration.shape.is_bounded_owned_byte_sequence()
                }))
            .then_some(result.structural_type)
        });
        let structural_type = matching.next()?;
        matching.next().is_none().then_some(structural_type)
    }

    pub(crate) fn owner_layout_type(
        function: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        owned_type(function, place).or_else(|| {
            let signature = function.structural.as_ref()?;
            let mut parameters = signature
                .parameters
                .iter()
                .map(|parameter| &parameter.semantic)
                .chain(function.block_structural_parameters())
                .filter(|parameter| parameter.place == place);
            let parameter = parameters.next()?;
            (parameters.next().is_none()
                && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                && parameter.projected_qualifications.is_empty()
                && signature.structural_types.iter().any(|declaration| {
                    declaration.id == parameter.structural_type
                        && declaration.shape.is_bounded_owned_byte_sequence()
                }))
            .then_some(parameter.structural_type)
        })
    }

    pub(crate) fn capacity(
        function: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
    ) -> Option<u32> {
        let LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
            result,
            left,
            right,
            ..
        } = &row.kind
        else {
            return None;
        };
        let signature = function.structural.as_ref()?;
        if row.result.is_some()
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.claims.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.projected_qualifications.is_empty()
            || !declares_operation_result(
                signature,
                result.place,
                row.operation,
                result.structural_type,
            )
        {
            return None;
        }
        let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
            signature.structural_type(result.structural_type)?.shape
        else {
            return None;
        };
        for operand in [left, right] {
            if operand.source() == result.place {
                return None;
            }
            match operand {
                ByteSequenceOperand::Whole { source, .. } => {
                    let root =
                        crate::selection::reference_input::root_type(function, source.place)?;
                    let geometry = crate::structural_inputs::structural_reference_input::descriptor_source_geometry(
                        root, &source.path, &signature.structural_types,
                    )?;
                    if source.access == StructuralAccess::WriteOnlyBorrow
                        || !geometry.readable
                        || !matches!(
                            signature.structural_type(geometry.selected)?.shape,
                            StructuralTypeShape::ByteSequence(_)
                        )
                    {
                        return None;
                    }
                }
                ByteSequenceOperand::Field { .. } => {
                    field_offset(function, operand)?;
                }
            }
        }
        u32::try_from(capacity).ok()?.checked_add(8)?;
        u32::try_from(capacity).ok()
    }

    pub(crate) fn field_offset(
        function: &LegalizedScalarFunction,
        operand: &ByteSequenceOperand,
    ) -> Option<u32> {
        let ByteSequenceOperand::Field { source, field, .. } = operand else {
            return None;
        };
        if source.access != StructuralAccess::SharedBorrow {
            return None;
        }
        crate::selection::field_bytes::FieldBytes::resolve(function, source, *field, &[])
            .map(|geometry| geometry.offset())
    }
}

pub(crate) mod byte_owner_input {
    use crate::selection::place_declaration::declares_block_parameter;

    use legalized_operations::LegalizedScalarFunction;
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};

    pub(crate) fn view_argument_at(
        function: &LegalizedScalarFunction,
        site: semantic_vocabulary::InvocationSite,
        semantic: &terminal_psi::StructuralArgument,
        target: &target_operations::TargetStructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Option<(u32, u64)> {
        crate::selection::scalar_call_abi::input_site::use_cut(function, site)?;
        let signature = function.structural.as_ref()?;
        if !matches!(
            semantic.access,
            terminal_psi::reading_borrow_pair_pattern!()
        ) || target.place != semantic.place
            || target.access != semantic.access
            || target.path != semantic.path
            || target.fixed_array_length.is_some()
            || target.element_stride.is_some()
            || target.shape != calling_conventions::ValueShape::borrowed_reference(16, 8)
        {
            return None;
        }
        let source = source_declaration(function, site, semantic, target)?;
        if target.root_structural_type != source.structural_type {
            return None;
        }
        let geometry = crate::structural_inputs::structural_reference_input::bounded_byte_view(
            &source,
            semantic,
            target.structural_type,
            &signature.structural_types,
        )?;
        (target.source_byte_offset == geometry.offset
            && crate::structural_inputs::runtime_indices_match(indices, &geometry.elements)
            && target.indices.len() == indices.len()
            && target.indices.iter().zip(indices).all(|(target, index)| {
                target.operand.source_value() == index.operand.value
                    && target.operand.scalar_type() == index.operand.scalar_type
                    && target.stride == index.stride
            }))
        .then_some((geometry.offset, geometry.capacity))
    }

    fn source_declaration(
        function: &LegalizedScalarFunction,
        site: semantic_vocabulary::InvocationSite,
        semantic: &terminal_psi::StructuralArgument,
        target: &target_operations::TargetStructuralArgument,
    ) -> Option<terminal_psi::StructuralParameterDeclaration> {
        let signature = function.structural.as_ref()?;
        match &target.source {
            target_operations::TargetStructuralArgumentSource::Placement(placement) => {
                let mut matching = signature
                    .parameters
                    .iter()
                    .filter(|parameter| parameter.semantic.place == semantic.place);
                let parameter = matching.next()?;
                if matching.next().is_some()
                    || parameter.target.placement != *placement
                    || parameter.semantic.multiplicity != StructuralMultiplicity::Unrestricted
                    || !matches!(
                        parameter.semantic.access,
                        StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
                    )
                {
                    return None;
                }
                Some(parameter.semantic.clone())
            }
            target_operations::TargetStructuralArgumentSource::BlockParameter { block, place } => {
                if *place != semantic.place || *block == function.entry_block {
                    return None;
                }
                let mut matching = function.blocks.iter().flat_map(|candidate| {
                    candidate
                        .structural_parameters
                        .iter()
                        .filter(|parameter| parameter.place == semantic.place)
                        .map(move |parameter| (candidate.id, parameter))
                });
                let (owner, parameter) = matching.next()?;
                if matching.next().is_some()
                    || owner != *block
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !matches!(
                        parameter.access,
                        StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
                    )
                    || !declares_block_parameter(signature, *place, *block, parameter.position)
                    || (parameter.access == StructuralAccess::Owned
                        && crate::selection::aggregate_result_input::block_home(function, *place)
                            != Some((*block, parameter)))
                    || (parameter.access != StructuralAccess::Owned
                        && crate::selection::address_join_input::join(function, *place)
                            != Some((*block, parameter)))
                {
                    return None;
                }
                Some(parameter.clone())
            }
            target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
                let (producer, result) =
                    crate::selection::aggregate_result_input::home(function, semantic.place)?;
                if producer != *psi_operation
                    || crate::selection::scalar_call_abi::input_site::producer_before_use(
                        function, producer, site,
                    )
                    .is_none()
                    || result.multiplicity != StructuralMultiplicity::Unrestricted
                {
                    return None;
                }
                Some(terminal_psi::StructuralParameterDeclaration {
                    place: result.place,
                    position: 0,
                    structural_type: result.structural_type,
                    multiplicity: result.multiplicity,
                    access: StructuralAccess::Owned,
                    qualifications: result.qualifications.clone(),
                    projected_qualifications: result.projected_qualifications.clone(),
                    additional_input_qualifications: result.additional_input_qualifications.clone(),
                })
            }
            target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                ..
            }
            | target_operations::TargetStructuralArgumentSource::EstablishedByteView { .. }
            | target_operations::TargetStructuralArgumentSource::EstablishedElementView {
                ..
            } => None,
        }
    }
}

pub(crate) mod established_view_input {
    use crate::selection::place_declaration::{
        declares_block_parameter, declares_operation_result,
    };
    use crate::selection::scalar_call_abi::input_site::{producer_before_use, use_cut};

    use legalized_operations::{
        LegalizedScalarArgument, LegalizedScalarFunction, LegalizedScalarInstructionKind,
        LegalizedScalarTerminator,
    };
    use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};
    use target_operations::{TargetStructuralArgument, TargetStructuralArgumentSource};
    use terminal_psi::StructuralTypeShape;

    pub(crate) mod access {
        use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind as I};
        use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
        use terminal_psi::{
            StructuralAccess as A, StructuralMultiplicity as M, StructuralTypeShape,
        };

        pub(crate) mod copied {
            use super::*;
            use crate::selection::aggregate_result_input::home;
            use crate::selection::primitive_local_input::read_parameter;
            use crate::structural_inputs::structural_reference_input as geometry;

            pub(crate) fn shared_projection(
                function: &LegalizedScalarFunction,
                row: &legalized_operations::LegalizedScalarInstruction,
            ) -> Option<()> {
                let I::StructuralLeafCopy {
                    result,
                    source,
                    path,
                    byte_offset,
                    shape,
                    indices,
                } = &row.kind
                else {
                    return None;
                };
                let signature = function.structural.as_ref()?;
                let types = &signature.structural_types;
                let root = if let Some(parameter) = read_parameter(function, *source) {
                    if parameter.access == A::WriteOnlyBorrow
                        || parameter.multiplicity == M::Linear
                        || !parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty()
                    {
                        return None;
                    }
                    parameter.structural_type
                } else {
                    home(function, *source)?.1.structural_type
                };
                let (selected, offset, elements, shared_field) = match path {
                    abstract_operations::StructuralCopyPath::Ordinary(path) => {
                        let geometry = geometry::leaf_copy_geometry(
                            root,
                            path,
                            result.structural_type,
                            types,
                        )?;
                        if !geometry.readable {
                            return None;
                        }
                        let shared_field = terminal_semantics::shared_byte_field_copy_predicates(
                            types.iter(),
                            root,
                            path,
                        )
                        .is_some();
                        (
                            geometry.selected,
                            geometry.byte_offset,
                            geometry.elements,
                            shared_field,
                        )
                    }
                    abstract_operations::StructuralCopyPath::Case(path) => {
                        let geometry = geometry::case_projection(root, path, types)?;
                        let shared_field = match path.split_last() {
                            Some((
                                semantic_vocabulary::CanonicalStructuralPathSegment::Field(field),
                                parent,
                            )) => terminal_semantics::observed_field_declaration(
                                types.iter(),
                                root,
                                parent,
                                *field,
                            )
                            .is_some_and(|field| {
                                matches!(
                                    field.field_type.byte_sequence_carrier(),
                                    Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                                        access: None | Some(A::SharedBorrow)
                                    })
                                )
                            }),
                            _ => false,
                        };
                        (
                            geometry.selected,
                            geometry.byte_offset,
                            geometry.elements,
                            shared_field,
                        )
                    }
                };
                if selected != result.structural_type
                    || offset != *byte_offset
                    || geometry::shape(selected, types)? != *shape
                    || !geometry::runtime_indices_match(indices, &elements)
                {
                    return None;
                }
                (shared_field
                    || types.iter().any(|declaration| {
                        declaration.id == selected
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                                        access: Some(A::SharedBorrow)
                                    }
                                ) | StructuralTypeShape::ElementView {
                                    access: Some(A::SharedBorrow),
                                    ..
                                }
                            )
                    }))
                .then_some(())
            }
        }

        fn supplies(actual: A, requested: A) -> bool {
            actual == A::Owned
                || matches!(
                    (actual, requested),
                    (A::MutableBorrow, A::SharedBorrow | A::MutableBorrow)
                        | (A::SharedBorrow, A::SharedBorrow)
                )
        }

        pub(crate) fn view_access(function: &LegalizedScalarFunction, place: PlaceId) -> Option<A> {
            let budget = function.structural.as_ref()?.structural_places.len();
            view_access_with_budget(function, place, budget)
        }

        fn same_descriptor_presentation(
            function: &LegalizedScalarFunction,
            actual: semantic_vocabulary::StructuralTypeId,
            expected: semantic_vocabulary::StructuralTypeId,
        ) -> bool {
            function.structural.as_ref().is_some_and(|signature| {
                terminal_semantics::borrowed_referent_presentation(
                    signature.structural_types.iter(),
                    actual,
                    expected,
                ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
            })
        }

        fn view_access_with_budget(
            function: &LegalizedScalarFunction,
            mut place: PlaceId,
            budget: usize,
        ) -> Option<A> {
            let signature = function.structural.as_ref()?;
            let mut required = A::SharedBorrow;
            let mut initial = None;
            let mut expected_type = None;
            let mut copied_whole = false;
            for remaining in (0..budget).rev() {
                if let Some((_, result)) = super::descriptor_result(function, place)
                    .or_else(|| super::descriptor_home(function, place))
                {
                    let access = terminal_semantics::descriptor_results::descriptor_value_access(
                        signature.structural_types.iter(),
                        result.structural_type,
                        result.multiplicity,
                    )?;
                    return (supplies(access, required)
                        && (!copied_whole || access == A::SharedBorrow)
                        && expected_type.is_none_or(|identity| {
                            same_descriptor_presentation(function, result.structural_type, identity)
                        }))
                    .then_some(initial.unwrap_or(access));
                }
                let mut declarations = signature
                    .structural_places
                    .iter()
                    .filter(|declaration| declaration.id == place);
                let declaration = declarations.next()?;
                if declarations.next().is_some() {
                    return None;
                }
                let mut parameters = signature
                    .parameters
                    .iter()
                    .map(|parameter| (None, &parameter.semantic))
                    .chain(function.blocks.iter().flat_map(|block| {
                        block
                            .structural_parameters
                            .iter()
                            .map(move |parameter| (Some(block.id), parameter))
                    }))
                    .filter(|(_, parameter)| parameter.place == place);
                if let Some((owner, parameter)) = parameters.next() {
                    let expected_kind = owner.map_or(
                        StructuralPlaceKind::Parameter {
                            position: parameter.position,
                        },
                        |block| StructuralPlaceKind::BlockParameter {
                            block,
                            position: parameter.position,
                        },
                    );
                    let access = terminal_semantics::descriptor_parameter_access(
                        parameter,
                        signature.structural_types.iter(),
                    )?;
                    if parameters.next().is_some()
                        || declaration.kind != expected_kind
                        || expected_type.is_some_and(|identity| {
                            !same_descriptor_presentation(
                                function,
                                parameter.structural_type,
                                identity,
                            )
                        })
                        || (!parameter.qualifications.is_empty()
                            && !(access == A::SharedBorrow
                                && signature.structural_types.iter().any(|row| {
                                    row.id == parameter.structural_type
                                        && row.shape.is_borrowed_byte_carrier()
                                })))
                        || (copied_whole && access != A::SharedBorrow)
                        || !supplies(access, required)
                    {
                        return None;
                    }
                    return Some(initial.unwrap_or(access));
                }
                if let StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                } = declaration.kind
                {
                    if required != A::SharedBorrow
                        || expected_type.is_some_and(|expected| !same_descriptor_presentation(function, structural_type, expected))
                        || !signature.structural_types.iter().any(|row|
                            row.id == structural_type && row.shape.is_borrowed_byte_view())
                        || !function.instructions().any(|row|
                            matches!(&row.kind, I::EstablishByteSequenceLiteral { destination, structural_type: actual, .. }
                                if destination == declaration && actual.id == structural_type)) {
                        return None;
                    }
                    return Some(initial.unwrap_or(A::SharedBorrow));
                }
                let (producer, structural_type) = declaration.kind.as_operation_result()?;
                if expected_type.is_some_and(|identity| {
                    !same_descriptor_presentation(function, structural_type, identity)
                }) || !signature.structural_types.iter().any(|row| {
                    row.id == structural_type
                        && (matches!(row.shape, StructuralTypeShape::ElementView { .. })
                            || row.shape.is_borrowed_byte_carrier())
                }) {
                    return None;
                }
                let mut producers = function
                    .instructions()
                    .filter(|row| row.operation == producer);
                let row = producers.next()?;
                if producers.next().is_some() || row.result.is_some() {
                    return None;
                }
                let result = match &row.kind {
                    I::ByteSequenceSubslice { result, .. }
                    | I::ElementViewSubslice { result, .. }
                    | I::EstablishElementView { result, .. }
                    | I::StructuralLeafCopy { result, .. } => result,
                    _ => return None,
                };
                if result.place != place
                    || result.structural_type != structural_type
                    || result
                        .projected_qualifications
                        .iter()
                        .any(|row| !row.is_element_membership())
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
                {
                    return None;
                }
                let access = terminal_semantics::descriptor_results::descriptor_value_access(
                    signature.structural_types.iter(),
                    result.structural_type,
                    result.multiplicity,
                )?;
                if !supplies(access, required) || (copied_whole && access != A::SharedBorrow) {
                    return None;
                }
                initial.get_or_insert(access);
                match &row.kind {
                    I::StructuralLeafCopy {
                        source,
                        path,
                        byte_offset,
                        shape,
                        indices,
                        ..
                    } => {
                        if access != A::SharedBorrow {
                            return None;
                        }
                        if matches!(path, abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty())
                        {
                            if *byte_offset != 0
                                || !indices.is_empty()
                                || crate::structural_inputs::structural_reference_input::shape(
                                    structural_type,
                                    &signature.structural_types,
                                )? != *shape
                            {
                                return None;
                            }
                            required = A::SharedBorrow;
                            expected_type = Some(structural_type);
                            copied_whole = true;
                            place = *source;
                        } else {
                            copied::shared_projection(function, row)?;
                            return initial;
                        }
                    }
                    I::ByteSequenceSubslice { source, .. }
                    | I::ElementViewSubslice { source, .. } => {
                        projected_source_access(function, source, access, remaining)?;
                        return initial;
                    }
                    I::EstablishElementView {
                        source,
                        destination,
                        element,
                        ..
                    } => {
                        if *destination != place || source.access != access
                            || !signature.structural_types.iter().any(|row| row.id == structural_type && (
                                matches!(row.shape, StructuralTypeShape::ElementView { element: declared, .. } if declared == *element)
                                || row.shape.is_borrowed_byte_carrier() && signature.structural_types.iter().any(|leaf|
                                    leaf.id == *element && matches!(leaf.shape,
                                        StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(integer))
                                        if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8))))) {
                            return None;
                        }
                        projected_source_access(function, source, access, remaining)?;
                        return initial;
                    }
                    _ => return None,
                }
            }
            None
        }

        fn projected_source_access(
            function: &LegalizedScalarFunction,
            source: &terminal_psi::StructuralArgument,
            requested: A,
            budget: usize,
        ) -> Option<()> {
            use crate::structural_inputs::structural_reference_input as geometry;
            use optimization_unit_semantics::StructuralStorageProjection;
            let signature = function.structural.as_ref()?;
            let types = &signature.structural_types;
            let mut current = crate::selection::reference_input::root_type(function, source.place)?;
            let mut authority = if types.iter().any(|row| {
                row.id == current
                    && (matches!(row.shape, StructuralTypeShape::ElementView { .. })
                        || row.shape.is_borrowed_byte_carrier())
            }) {
                view_access_with_budget(function, source.place, budget)?
            } else if let Some(parameter) = signature
                .parameters
                .iter()
                .map(|parameter| &parameter.semantic)
                .chain(function.block_structural_parameters())
                .find(|parameter| parameter.place == source.place)
            {
                parameter.access
            } else {
                crate::selection::aggregate_result_input::home(function, source.place)?;
                A::Owned
            };
            if !supplies(source.access, requested) || authority == A::WriteOnlyBorrow {
                return None;
            }
            let mut prefix_start = 0;
            for (position, segment) in source.path.iter().enumerate() {
                if *segment != terminal_psi::StructuralPathSegment::Referent {
                    continue;
                }
                let (reference, _, _) = geometry::runtime_projection(
                    current,
                    &source.path[prefix_start..position],
                    types,
                )?;
                let StructuralTypeShape::Reference { referent, access } =
                    types.iter().find(|row| row.id == reference)?.shape
                else {
                    return None;
                };
                authority = if supplies(authority, access) {
                    access
                } else if supplies(access, authority) {
                    authority
                } else {
                    return None;
                };
                if authority == A::WriteOnlyBorrow {
                    return None;
                }
                current = referent;
                prefix_start = position + 1;
            }
            let selected = optimization_unit_semantics::structural_borrowed_storage_projection(
                types.iter(),
                current,
                &source.path[prefix_start..],
                requested,
            )?;
            let ceiling = match selected {
                StructuralStorageProjection::ByteField { carrier, .. } => match carrier {
                    terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                    | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. } => {
                        access.unwrap_or(A::SharedBorrow)
                    }
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => A::Owned,
                },
                StructuralStorageProjection::Endpoint(identity) => {
                    match types.iter().find(|row| row.id == identity)?.shape {
                        StructuralTypeShape::ElementView { access, .. } => access.unwrap_or({
                            if source.path.len() == prefix_start {
                                authority
                            } else {
                                A::SharedBorrow
                            }
                        }),
                        StructuralTypeShape::FixedArray { .. } => authority,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                            | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
                        ) => access.unwrap_or(authority),
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                        ) => A::Owned,
                        _ => return None,
                    }
                }
                StructuralStorageProjection::CollectionWindow { .. } => return None,
            };
            (supplies(authority, requested) && supplies(ceiling, requested)).then_some(())
        }
    }
    pub(crate) use access::view_access;

    pub(crate) fn requires_descriptor(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
        transferred(source, place)
            || source.blocks.iter().any(|block| matches!(&block.terminator,
                LegalizedScalarTerminator::InvokeNoNormal { call, .. } if call.arguments.iter().any(|argument|
                    matches!(argument, LegalizedScalarArgument::Structural { semantic, .. }
                        if semantic.place == place))))
            || source.blocks.iter().any(|block| matches!(&block.terminator,
                LegalizedScalarTerminator::Return(returned) | LegalizedScalarTerminator::ReturnCleanup { returned, .. } if matches!(&returned.value,
                    legalized_operations::LegalizedScalarReturnValue::SharedDescriptor { place: returned } if *returned == place)
                    || matches!(&returned.value,
                    legalized_operations::LegalizedScalarReturnValue::Structural { source: legalized_operations::LegalizedStructuralCaseSource::OperationResult { result, .. } } if result.place == place)))
            || source.instructions()
                .any(|row| match &row.kind {
                    LegalizedScalarInstructionKind::Call(call) => call.arguments.iter().any(|argument|
                        matches!(argument, LegalizedScalarArgument::Structural { semantic, .. }
                            if semantic.place == place)),
                    LegalizedScalarInstructionKind::NormalizedForeignCall(call) => call
                        .structural_arguments.iter().any(|argument| argument.place == place),
                    LegalizedScalarInstructionKind::EstablishReference { source: argument, .. }
                    | LegalizedScalarInstructionKind::EstablishAccessReference { source: argument, .. } =>
                        argument.place == place
                            && crate::selection::reference_input::establishment(source, row).is_some_and(|address|
                                address.place == place && address.pointer_slots.is_empty()),
                    LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
                    | LegalizedScalarInstructionKind::ElementViewLength { source, .. }
                    | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. }
                    | LegalizedScalarInstructionKind::ElementViewSubslice { source, .. }
                    | LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, .. }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { source, .. }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead { source, .. } =>
                        source.place == place && !source.path.is_empty(),
                    LegalizedScalarInstructionKind::ActivateTaskRegister { gdt, task_state, scratch, .. } =>
                        [gdt, task_state, scratch].into_iter().any(|argument| argument.place == place && !argument.path.is_empty()),
    LegalizedScalarInstructionKind::ReloadSegmentDescriptors { gdt, scratch, .. } =>
                        [gdt, scratch].into_iter().any(|argument| argument.place == place && !argument.path.is_empty()),
                    LegalizedScalarInstructionKind::StorageAddress { source, path, .. }
                    | LegalizedScalarInstructionKind::GlobalDescriptorTable { source, path, .. }
                    | LegalizedScalarInstructionKind::PrimitiveScalarRead { source, path, .. } =>
                        *source == place && !path.is_empty(),
                    LegalizedScalarInstructionKind::EstablishRecord { fields, .. }
                    | LegalizedScalarInstructionKind::EstablishCase { fields, .. } => fields.iter().any(|field|
                        matches!(&field.value, terminal_psi::RecordFieldValue::Structural(argument)
                            if argument.place == place)),
                    LegalizedScalarInstructionKind::EstablishStructuralArray { elements, .. } =>
                        elements.iter().any(|argument| argument.place == place),
                    LegalizedScalarInstructionKind::StructuralByteViewFieldStore { source, .. }
                    | LegalizedScalarInstructionKind::StructuralLeafCopy { source, .. } => *source == place,
                    LegalizedScalarInstructionKind::StoreStructuralField { value, .. } => *value == place,
                    _ => false,
                })
    }

    pub(crate) fn transferred(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
        source.blocks.iter().any(|block| {
            let successors = match &block.terminator {
                legalized_operations::LegalizedScalarTerminator::Crash { .. }
                | legalized_operations::LegalizedScalarTerminator::InvokeNoNormal { .. }
                | legalized_operations::LegalizedScalarTerminator::ExternalCompletion { .. }
                | legalized_operations::LegalizedScalarTerminator::Return(_)
                | legalized_operations::LegalizedScalarTerminator::ReturnCleanup { .. }
                | legalized_operations::LegalizedScalarTerminator::StructuralCase { .. } => {
                    [None, None]
                }
                legalized_operations::LegalizedScalarTerminator::Jump { successor, .. } => {
                    [Some(successor), None]
                }
                legalized_operations::LegalizedScalarTerminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => [Some(when_true), Some(when_false)],
            };
            successors.into_iter().flatten().any(|successor| {
                successor
                    .structural_bindings
                    .iter()
                    .any(|binding| binding.semantic.argument.place == place)
            })
        })
    }

    pub(crate) fn view_type(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        declared_view_type(source, place).filter(|identity| {
            source.structural.as_ref().is_some_and(|signature| {
                signature.structural_types.iter().any(|declaration| {
                    declaration.id == *identity && declaration.shape.is_borrowed_byte_carrier()
                })
            })
        })
    }

    pub(crate) fn element_view_type(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        declared_view_type(source, place).filter(|identity| {
            source.structural.as_ref().is_some_and(|signature| {
                signature.structural_types.iter().any(|declaration| {
                    declaration.id == *identity
                        && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                })
            })
        })
    }

    fn declared_view_type(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        let signature = source.structural.as_ref()?;
        let parameter = signature
            .parameters
            .iter()
            .map(|parameter| &parameter.semantic)
            .chain(source.block_structural_parameters())
            .find(|parameter| parameter.place == place);
        if let Some(parameter) = parameter {
            terminal_semantics::descriptor_parameter_access(
                parameter,
                signature.structural_types.iter(),
            )?;
            return Some(parameter.structural_type);
        }
        source.instructions().find_map(|row| {
            if let Some(result) = row.kind.get_structural_result()
                && result.place == place
            {
                return Some(result.structural_type);
            }
            match &row.kind {
                LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                    destination,
                    structural_type,
                    ..
                } if destination.id == place => Some(structural_type.id),
                _ => None,
            }
        })
    }

    pub(crate) fn accepts(
        source: &LegalizedScalarFunction,
        call: OperationId,
        target: &TargetStructuralArgument,
    ) -> Option<()> {
        accepts_at(
            source,
            semantic_vocabulary::InvocationSite::Operation(call),
            target,
        )
    }

    pub(crate) fn accepts_at(
        source: &LegalizedScalarFunction,
        call: semantic_vocabulary::InvocationSite,
        target: &TargetStructuralArgument,
    ) -> Option<()> {
        use_cut(source, call)?;
        if let TargetStructuralArgumentSource::BlockParameter { block, place } = target.source {
            let owner = source.block(block)?;
            let parameter = owner.structural_parameter(place)?;
            let signature = source.structural.as_ref()?;
            let access = terminal_semantics::descriptor_parameter_access(
                parameter,
                signature.structural_types.iter(),
            )?;
            return (place == target.place
                && parameter.structural_type == target.root_structural_type
                && matches!(
                    access,
                    terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::MutableBorrow
                )
                && (target.access == access
                    || target.access == terminal_psi::StructuralAccess::SharedBorrow
                        && access == terminal_psi::StructuralAccess::MutableBorrow)
                && terminal_semantics::view_descriptor_projected_qualifications(
                    signature.structural_types.iter(),
                    parameter.structural_type,
                    &parameter.projected_qualifications,
                )
                && declares_block_parameter(signature, place, block, parameter.position))
            .then_some(());
        }
        let (psi_operation, element_view_source) = match &target.source {
            TargetStructuralArgumentSource::EstablishedByteView { psi_operation } => {
                (*psi_operation, false)
            }
            TargetStructuralArgumentSource::EstablishedElementView { psi_operation } => {
                (*psi_operation, true)
            }
            _ => return None,
        };
        {
            let actual = view_access(source, target.place)?;
            if target.access != actual
                && !(target.access == terminal_psi::StructuralAccess::SharedBorrow
                    && actual == terminal_psi::StructuralAccess::MutableBorrow)
            {
                return None;
            }
        }
        let signature = source.structural.as_ref()?;
        let producer = producer_before_use(source, psi_operation, call)?;
        if producer.2.result.is_some() {
            return None;
        }
        if let Some(result) = producer.2.kind.get_structural_result()
            && !terminal_semantics::borrowed_referent_matches(
                signature.structural_types.iter(),
                result.structural_type,
                target.structural_type,
            )
        {
            return None;
        }
        match &producer.2.kind {
            LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                destination,
                structural_type,
                ..
            } => {
                if destination.id != target.place
                    || element_view_source
                    || !signature.structural_places.contains(destination)
                    || !signature.structural_types.contains(structural_type)
                    || structural_type.id != target.root_structural_type
                    || terminal_semantics::borrowed_referent_presentation(
                        signature.structural_types.iter(),
                        structural_type.id,
                        target.structural_type,
                    ) != Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                    || !structural_type.shape.is_borrowed_byte_view()
                    || !matches!(destination.kind, StructuralPlaceKind::ByteSequenceLiteral { structural_type: identity, .. } if identity == structural_type.id)
                {
                    return None;
                }
            }
            LegalizedScalarInstructionKind::ByteSequenceSubslice { result, .. } => {
                if result.place != target.place
                    || !result.qualifications.is_empty()
                    || !terminal_semantics::view_descriptor_projected_qualifications(signature.structural_types.iter(), result.structural_type, &result.projected_qualifications)
                    || !result.claims.is_empty()
                    || element_view_source
                    || !declares_operation_result(
                        signature,
                        result.place,
                        psi_operation,
                        result.structural_type,
                    )
                    || !signature.structural_types.iter().any(|declaration| {
                        declaration.id == result.structural_type
                            && declaration.shape.is_borrowed_byte_view()
                    })
                {
                    return None;
                }
            }
            LegalizedScalarInstructionKind::ElementViewSubslice { result, .. } => {
                if result.place != target.place
                    || !result.qualifications.is_empty()
                    || !terminal_semantics::view_descriptor_projected_qualifications(signature.structural_types.iter(), result.structural_type, &result.projected_qualifications)
                    || !result.claims.is_empty()
                    || !element_view_source
                    || !declares_operation_result(
                        signature,
                        result.place,
                        psi_operation,
                        result.structural_type,
                    )
                    || !signature.structural_types.iter().any(|declaration| {
                        declaration.id == result.structural_type
                            && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                    })
                {
                    return None;
                }
            }
            LegalizedScalarInstructionKind::EstablishElementView {
                result,
                destination,
                element,
                ..
            } => {
                if result.place != target.place
                    || *destination != target.place
                    || !result.qualifications.is_empty()
                    || !terminal_semantics::view_descriptor_projected_qualifications(signature.structural_types.iter(), result.structural_type, &result.projected_qualifications)
                    || !result.claims.is_empty()
                    || !declares_operation_result(
                        signature,
                        result.place,
                        psi_operation,
                        result.structural_type,
                    )
                    || !signature.structural_types.iter().any(|declaration| {
                        declaration.id == result.structural_type
                            && if element_view_source {
                                matches!(declaration.shape, StructuralTypeShape::ElementView { element: declared, .. } if declared == *element)
                            } else {
                                declaration.shape.is_borrowed_byte_carrier()
                            }
                    })
                {
                    return None;
                }
            }
            _ => return None,
        }
        Some(())
    }

    pub(crate) fn accepts_view_loan(
        source: &LegalizedScalarFunction,
        call: semantic_vocabulary::InvocationSite,
        target: &TargetStructuralArgument,
    ) -> Option<()> {
        let TargetStructuralArgumentSource::EstablishedElementView { psi_operation } =
            target.source
        else {
            return None;
        };
        let access = view_access(source, target.place)?;
        if target.access != access
            && !(access == terminal_psi::StructuralAccess::MutableBorrow
                && matches!(
                    target.access,
                    terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                ))
        {
            return None;
        }
        let signature = source.structural.as_ref()?;
        let producer = producer_before_use(source, psi_operation, call)?;
        if producer.2.result.is_some() {
            return None;
        }
        let (result, destination) = match &producer.2.kind {
            LegalizedScalarInstructionKind::EstablishElementView {
                result,
                destination,
                ..
            } => (result, Some(*destination)),
            LegalizedScalarInstructionKind::ElementViewSubslice { result, .. } => (result, None),
            _ => return None,
        };
        (destination.is_none_or(|destination| destination == target.place)
            && result.place == target.place
            && result.structural_type == target.root_structural_type
            && matches!(
                result.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            )
            && result.qualifications.is_empty()
            && terminal_semantics::view_descriptor_projected_qualifications(
                signature.structural_types.iter(),
                result.structural_type,
                &result.projected_qualifications,
            )
            && result.claims.is_empty()
            && declares_operation_result(
                signature,
                result.place,
                psi_operation,
                result.structural_type,
            )
            && signature.structural_types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            }))
        .then_some(())
    }

    pub(crate) fn readable_byte_type(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        view_type(source, place).or_else(|| {
            crate::selection::byte_concatenation_input::owner_layout_type(source, place)
        })
    }

    pub(crate) fn completed_descriptor(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<(OperationId, &terminal_psi::StructuralOperationResult)> {
        let (operation, result) =
            crate::selection::aggregate_result_input::operation_result(source, place)?;
        let signature = source.structural.as_ref()?;
        (matches!(
            result.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        ) && terminal_semantics::view_descriptor_projected_qualifications(
            signature.structural_types.iter(),
            result.structural_type,
            &result.projected_qualifications,
        ) && result.qualification_establishments.is_empty()
            && result.claims.is_empty()
            && terminal_semantics::descriptor_results::is_descriptor(
                signature.structural_types.iter(),
                result.structural_type,
            ))
        .then_some((operation, result))
    }

    pub(crate) fn descriptor_home(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<(OperationId, &terminal_psi::StructuralOperationResult)> {
        let (operation, result) = completed_descriptor(source, place)?;
        let row = source
            .instructions()
            .find(|row| row.operation == operation)?;
        matches!(
            &row.kind,
            LegalizedScalarInstructionKind::EstablishStructuralLocal { result: established, .. }
                if established == result
        )
        .then_some((operation, result))
    }

    pub(crate) fn descriptor_result(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<(OperationId, &terminal_psi::StructuralOperationResult)> {
        let (operation, result) = completed_descriptor(source, place)?;
        let row = source
            .instructions()
            .find(|row| row.operation == operation)?;
        let LegalizedScalarInstructionKind::Call(call) = &row.kind else {
            return None;
        };
        if call.descriptor_origins.is_empty()
            || call.structural_result.as_ref() != Some(result)
            || crate::selection::aggregate_result_input::call_result(source, call)
                .map(|(result, _)| result)
                != Some(result)
        {
            return None;
        }
        Some((operation, result))
    }
}

pub(crate) mod literal_compare_input {
    use std::ops::RangeInclusive;

    use legalized_operations::{
        LegalizedScalarBlock, LegalizedScalarComparison, LegalizedScalarFunction,
        LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

    fn folded_literal<'a>(
        function: &LegalizedScalarFunction,
        block: &'a LegalizedScalarBlock,
        comparison_index: usize,
        immediates: RangeInclusive<u64>,
    ) -> Option<&'a LegalizedScalarInstruction> {
        let comparison = block.instructions.get(comparison_index)?;
        let literal = block.instructions.get(comparison_index.checked_sub(1)?)?;
        let definition = literal.result?;
        let LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(value)) = literal.kind
        else {
            return None;
        };
        let value = u64::try_from(value).ok()?;
        if !matches!(
            definition.scalar_type,
            ScalarType::Integer(integer)
                if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64
        ) || !immediates.contains(&value)
        {
            return None;
        }
        let LegalizedScalarInstructionKind::Compare {
            predicate: LegalizedScalarComparison::Equal,
            operand_type,
            right_operand_type,
            left,
            right,
            ..
        } = comparison.kind
        else {
            return None;
        };
        if operand_type != right_operand_type {
            return None;
        }
        if (left == definition.value) == (right == definition.value) {
            return None;
        }
        for source_block in &function.blocks {
            if source_block.terminator.references_value(definition.value) {
                return None;
            }
            for instruction in &source_block.instructions {
                if instruction.operation == comparison.operation {
                    continue;
                }
                if instruction.references_value(definition.value) {
                    return None;
                }
            }
        }
        Some(literal)
    }

    pub(crate) fn folded_zero<'a>(
        function: &LegalizedScalarFunction,
        block: &'a LegalizedScalarBlock,
        comparison_index: usize,
    ) -> Option<&'a LegalizedScalarInstruction> {
        folded_literal(function, block, comparison_index, 0..=0)
    }

    pub(crate) fn folded_immediate<'a>(
        function: &LegalizedScalarFunction,
        block: &'a LegalizedScalarBlock,
        comparison_index: usize,
    ) -> Option<&'a LegalizedScalarInstruction> {
        folded_literal(function, block, comparison_index, 1..=4095)
    }
}

pub(crate) mod native_code_layout_input {
    use crate::structural_inputs::structural_reference_input as geometry;
    use calling_conventions::ValueShape;
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use terminal_psi::{
        StructuralFieldType, StructuralRecordStorage, StructuralTypeShape, find_structural_type,
    };

    pub(crate) fn valid(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
    ) -> bool {
        let LegalizedScalarInstructionKind::EstablishNativeCodeLayout { result, shape, .. } =
            &row.kind
        else {
            return false;
        };
        let Some(signature) = &source.structural else {
            return false;
        };
        let types = &signature.structural_types;
        let Some(descriptor) = find_structural_type(types, result.structural_type) else {
            return false;
        };
        let StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::Packed,
        } = &descriptor.shape
        else {
            return false;
        };
        if fields.len() != 2
            || fields
                .iter()
                .any(|field| field.relevance.is_erased() || field.storage.is_some())
            || *shape != ValueShape::integer(24, 8)
            || geometry::shape(result.structural_type, types) != Some(*shape)
        {
            return false;
        }
        let StructuralFieldType::Scalar(address) = fields[0].field_type else {
            return false;
        };
        if !matches!(address, semantic_vocabulary::ScalarType::Integer(ty) if ty.is_address() && ty.bits() == 64)
            || geometry::field_read(result.structural_type, &[], fields[0].id, address, types)
                != Some((0, 8))
        {
            return false;
        }
        let StructuralFieldType::Structural(view) = fields[1].field_type else {
            return false;
        };
        let Some(view_type) = find_structural_type(types, view) else {
            return false;
        };
        let StructuralTypeShape::ElementView { element, access } = view_type.shape else {
            return false;
        };
        if access.is_some_and(|access| access != terminal_psi::StructuralAccess::SharedBorrow)
            || geometry::shape(view, types) != Some(ValueShape::integer(16, 8))
        {
            return false;
        }
        let Some(view_shape) = geometry::field_storage_shape(&fields[1], types) else {
            return false;
        };
        let view_offset = (8u32 + u32::from(view_shape.alignment) - 1)
            / u32::from(view_shape.alignment)
            * u32::from(view_shape.alignment);
        if view_offset != 8 || view_shape != ValueShape::integer(16, 8) {
            return false;
        }
        let Some(region_type) = find_structural_type(types, element) else {
            return false;
        };
        let StructuralTypeShape::Record {
            fields: region,
            storage: StructuralRecordStorage::Packed,
        } = &region_type.shape
        else {
            return false;
        };
        if region.len() != 2
            || region
                .iter()
                .any(|field| field.relevance.is_erased() || field.storage.is_some())
            || geometry::shape(element, types) != Some(ValueShape::integer(16, 8))
        {
            return false;
        }
        let StructuralFieldType::Scalar(base) = region[0].field_type else {
            return false;
        };
        let StructuralFieldType::Scalar(length) = region[1].field_type else {
            return false;
        };
        base == address
            && matches!(length, semantic_vocabulary::ScalarType::Integer(ty) if !ty.is_address() && ty.bits() == 64 && ty.sign() == semantic_vocabulary::IntegerSign::Unsigned)
            && geometry::field_read(element, &[], region[0].id, base, types) == Some((0, 8))
            && geometry::field_read(element, &[], region[1].id, length, types) == Some((8, 8))
    }
}

pub(crate) mod owned_block_home_input {
    use crate::selection::place_declaration::declares_block_parameter;

    use legalized_operations::LegalizedScalarFunction;
    use semantic_vocabulary::BlockId;
    use terminal_psi::StructuralParameterDeclaration;

    pub(crate) fn store_destination(
        source: &LegalizedScalarFunction,
        block: BlockId,
        expected: &StructuralParameterDeclaration,
    ) -> bool {
        let Some(signature) = source.structural.as_ref() else {
            return false;
        };
        let mut matching = source.blocks.iter().flat_map(|candidate| {
            candidate
                .structural_parameters
                .iter()
                .filter(|parameter| parameter.place == expected.place)
                .map(move |parameter| (candidate.id, parameter))
        });
        matching.next() == Some((block, expected))
            && matching.next().is_none()
            && block != source.entry_block
            && crate::selection::address_join_input::join(source, expected.place).is_none()
            && crate::selection::aggregate_result_input::block_parameter_shape(source, expected)
                .is_some()
            && declares_block_parameter(signature, expected.place, block, expected.position)
    }
}

pub(crate) mod primitive_local_input {
    use crate::selection::place_declaration::{
        arrivals, declares_block_parameter, declares_operation_result,
    };
    use crate::structural_inputs::structural_reference_input::{
        PointerSlot, RuntimeElement, case_field_read, descriptor_source_geometry,
        element_view_length_geometry, field_read, primitive_read_geometry, projected_byte_field,
        projected_scalar_codec, projected_scalar_fragments, scalar_field_codec,
        scalar_field_fragments, storage_address_geometry,
    };

    use calling_conventions::ValueShape;
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstructionKind as Instruction,
    };
    use semantic_vocabulary::{OperationId, PlaceId, ScalarType};
    use terminal_psi::{
        StructuralAccess, StructuralMultiplicity, StructuralOperationResult, StructuralTypeShape,
    };

    pub(crate) fn local(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<(
        OperationId,
        &StructuralOperationResult,
        ScalarType,
        ValueShape,
    )> {
        let signature = source.structural.as_ref()?;
        let mut candidates = source.instructions().filter_map(|row| match &row.kind {
            Instruction::EstablishPrimitiveLocal {
                result,
                value,
                shape,
            } if result.place == place => Some((row, result, value, shape)),
            _ => None,
        });
        let (row, result, value, shape) = candidates.next()?;
        if candidates.next().is_some()
            || row.result.is_some()
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualification_establishments.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !declares_operation_result(signature, place, row.operation, result.structural_type)
            || !signature.structural_types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && declaration.shape == StructuralTypeShape::PrimitiveScalar(value.scalar_type)
            })
            || crate::selection::scalar_call_abi::scalar_shape(value.scalar_type) != Some(*shape)
        {
            return None;
        }
        Some((row.operation, result, value.scalar_type, *shape))
    }
    pub(crate) fn read_parameter(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<&terminal_psi::StructuralParameterDeclaration> {
        source
            .structural
            .as_ref()?
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == place)
            .map(|parameter| &parameter.semantic)
            .or_else(|| block_arrival(source, place))
    }

    fn block_arrival(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<&terminal_psi::StructuralParameterDeclaration> {
        let signature = source.structural.as_ref()?;
        let mut matching = arrivals(source, place);
        let (block, parameter) = matching.next()?;
        (matching.next().is_none()
            && block != source.entry_block
            && declares_block_parameter(signature, place, block, parameter.position))
        .then_some(parameter)
    }

    fn projected_read_root(
        source: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<semantic_vocabulary::StructuralTypeId> {
        crate::selection::aggregate_result_input::operation_result(source, place)
            .map(|(_, result)| result.structural_type)
            .or_else(|| read_parameter(source, place).map(|parameter| parameter.structural_type))
    }

    pub(crate) fn read_storage_codec(
        source: &LegalizedScalarFunction,
        row: &legalized_operations::LegalizedScalarInstruction,
    ) -> Option<runtime_abi::ScalarStorageCodec> {
        let scalar = row.result?.scalar_type;
        let signature = source.structural.as_ref()?;
        match &row.kind {
            Instruction::PrimitiveScalarRead {
                source: place,
                path,
                ..
            } => {
                if local(source, *place).is_some() {
                    return runtime_abi::ScalarStorageCodec::from_scalar(scalar);
                }
                let root = projected_read_root(source, *place)?;
                projected_scalar_codec(root, path, &signature.structural_types)
            }
            Instruction::StructuralScalarFieldRead {
                source: argument,
                field,
                case: None,
            } => {
                let root = projected_read_root(source, argument.place)?;
                scalar_field_codec(root, &argument.path, *field, &signature.structural_types)
            }
            _ => runtime_abi::ScalarStorageCodec::from_scalar(scalar),
        }
    }

    pub(crate) fn read_storage_fragments<'source>(
        source: &'source LegalizedScalarFunction,
        row: &legalized_operations::LegalizedScalarInstruction,
    ) -> Option<&'source semantic_vocabulary::AuthoredBitFragments> {
        let signature = source.structural.as_ref()?;
        match &row.kind {
            Instruction::PrimitiveScalarRead {
                source: place,
                path,
                ..
            } => {
                let root = projected_read_root(source, *place)?;
                projected_scalar_fragments(root, path, &signature.structural_types)
            }
            Instruction::StructuralScalarFieldRead {
                source: argument,
                field,
                case: None,
            } => {
                let root = projected_read_root(source, argument.place)?;
                scalar_field_fragments(root, &argument.path, *field, &signature.structural_types)
            }
            _ => None,
        }
    }

    fn length_count_matches(
        scalar: ScalarType,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
        elements: &[RuntimeElement],
    ) -> Option<()> {
        let count = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .ok()?,
        );
        (scalar == count && crate::structural_inputs::runtime_indices_match(indices, elements))
            .then_some(())
    }

    pub(crate) fn read_geometry(
        source: &LegalizedScalarFunction,
        row: &legalized_operations::LegalizedScalarInstruction,
    ) -> Option<(PlaceId, u32, Vec<PointerSlot>)> {
        if let Instruction::GlobalDescriptorTable {
            source: place,
            path,
            indices,
            ..
        } = &row.kind
        {
            let signature = source.structural.as_ref()?;
            let root = crate::selection::reference_input::root_type(source, *place)?;
            let geometry = storage_address_geometry(root, path, &signature.structural_types)?;
            return crate::structural_inputs::runtime_indices_match(indices, &geometry.elements)
                .then_some((*place, geometry.byte_offset, geometry.pointer_slots));
        }
        let scalar = row.result?.scalar_type;
        match &row.kind {
            Instruction::ByteSequenceLength {
                source: argument,
                indices,
                length_byte_offset,
            } => {
                let signature = source.structural.as_ref()?;
                let root = crate::selection::reference_input::root_type(source, argument.place)?;
                let mut geometry =
                    descriptor_source_geometry(root, &argument.path, &signature.structural_types)?;
                let terminal_psi::StructuralTypeShape::ByteSequence(carrier) =
                    terminal_psi::find_structural_type(
                        &signature.structural_types,
                        geometry.selected,
                    )?
                    .shape
                else {
                    return None;
                };
                let expected = match carrier {
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => 8,
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => 0,
                    terminal_psi::ByteSequenceCarrier::BoundedView { .. } => return None,
                };
                geometry.byte_offset = geometry.byte_offset.checked_add(expected)?;
                (geometry.readable
                    && *length_byte_offset == expected
                    && crate::structural_inputs::runtime_indices_match(indices, &geometry.elements))
                .then_some((argument.place, geometry.byte_offset, geometry.pointer_slots))
            }
            Instruction::ElementViewLength {
                source: argument,
                indices,
                length_byte_offset,
            } => {
                if *length_byte_offset != 8 {
                    return None;
                }
                let signature = source.structural.as_ref()?;
                let root = crate::selection::reference_input::root_type(source, argument.place)?;
                let geometry = element_view_length_geometry(
                    root,
                    &argument.path,
                    &signature.structural_types,
                )?;
                length_count_matches(scalar, indices, &geometry.elements)?;
                Some((argument.place, geometry.byte_offset, geometry.pointer_slots))
            }
            Instruction::StructuralByteSequenceFieldLength {
                source: argument,
                field,
                indices,
            } => {
                let signature = source.structural.as_ref()?;
                let root = crate::selection::field_bytes::root_type(source, argument)?;
                let geometry = projected_byte_field(
                    root,
                    &argument.path,
                    *field,
                    &signature.structural_types,
                )?;
                length_count_matches(scalar, indices, &geometry.elements)?;
                Some((
                    argument.place,
                    geometry.length_offset,
                    geometry.pointer_slots,
                ))
            }

            Instruction::StorageAddress {
                source: place,
                path,
                indices,
            } => {
                if !matches!(scalar, ScalarType::Integer(integer) if integer.is_address()) {
                    return None;
                }
                let signature = source.structural.as_ref()?;
                let root = crate::selection::reference_input::root_type(source, *place)?;
                let geometry = storage_address_geometry(root, path, &signature.structural_types)?;
                crate::structural_inputs::runtime_indices_match(indices, &geometry.elements)
                    .then_some((*place, geometry.byte_offset, geometry.pointer_slots))
            }
            Instruction::PrimitiveScalarRead {
                source: place,
                path,
                indices,
            } => {
                crate::selection::scalar_call_abi::scalar_shape(scalar)?;
                if let Some((_, _, actual, _)) = local(source, *place) {
                    return (path.is_empty() && indices.is_empty() && actual == scalar)
                        .then_some((*place, 0, Vec::new()));
                }
                let signature = source.structural.as_ref()?;
                let structural_type = if let Some((_, result)) =
                    crate::selection::aggregate_result_input::home(source, *place)
                {
                    result.structural_type
                } else {
                    let parameter = read_parameter(source, *place)?;
                    if !(matches!(
                        parameter.access,
                        terminal_psi::reading_borrow_pair_pattern!()
                    ) || parameter.access == StructuralAccess::Owned)
                    {
                        return None;
                    }
                    parameter.structural_type
                };
                let geometry = primitive_read_geometry(
                    structural_type,
                    path,
                    scalar,
                    &signature.structural_types,
                )?;
                crate::structural_inputs::runtime_indices_match(indices, &geometry.elements)
                    .then_some((*place, geometry.byte_offset, geometry.pointer_slots))
            }
            Instruction::StructuralScalarFieldRead {
                source: argument,
                field,
                ..
            } => {
                let signature = source.structural.as_ref()?;
                let structural_type = if let Some(parameter) =
                    read_parameter(source, argument.place)
                {
                    if parameter.access != argument.access
                        || parameter.access == StructuralAccess::WriteOnlyBorrow
                    {
                        return None;
                    }
                    parameter.structural_type
                } else {
                    let (_, result) = crate::selection::aggregate_result_input::operation_result(
                        source,
                        argument.place,
                    )?;
                    let expected_access = match signature
                        .structural_type(result.structural_type)?
                        .shape
                    {
                        StructuralTypeShape::Reference { access, .. } => access,
                        StructuralTypeShape::ElementView { .. } => StructuralAccess::SharedBorrow,
                        _ => StructuralAccess::Owned,
                    };
                    if argument.access != expected_access {
                        return None;
                    }
                    result.structural_type
                };
                let (structural_type, path) =
                    if let Some((terminal_psi::StructuralPathSegment::Referent, suffix)) =
                        argument.path.split_first()
                    {
                        let StructuralTypeShape::Reference {
                            referent,
                            access: access @ (terminal_psi::reading_borrow_pair_pattern!()),
                        } = signature.structural_type(structural_type)?.shape
                        else {
                            return None;
                        };
                        if argument.access != access {
                            return None;
                        }
                        (referent, suffix)
                    } else {
                        (structural_type, argument.path.as_slice())
                    };
                let (offset, _) = match &row.kind {
                    Instruction::StructuralScalarFieldRead {
                        case: Some(case), ..
                    } => case_field_read(
                        structural_type,
                        path,
                        *case,
                        *field,
                        scalar,
                        &signature.structural_types,
                    )?,
                    _ => field_read(
                        structural_type,
                        path,
                        *field,
                        scalar,
                        &signature.structural_types,
                    )?,
                };
                Some((argument.place, offset, Vec::new()))
            }
            _ => None,
        }
    }
}

pub(crate) mod record_input {
    use crate::selection::established_view_input::{element_view_type, view_access, view_type};
    use calling_conventions::ValueShape;
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use terminal_psi::{
        RecordFieldValue, StructuralFieldType, StructuralMultiplicity, StructuralTypeShape,
        find_structural_type,
    };

    pub(crate) fn declared_parameters<'source>(
        source: &'source LegalizedScalarFunction,
    ) -> Option<impl Iterator<Item = &'source terminal_psi::StructuralParameterDeclaration> + 'source>
    {
        let signature = source.structural.as_ref()?;
        Some(
            signature
                .parameters
                .iter()
                .map(|parameter| &parameter.semantic)
                .chain(
                    source
                        .blocks
                        .iter()
                        .filter(|block| block.id != source.entry_block)
                        .flat_map(|block| &block.structural_parameters),
                ),
        )
    }

    pub(crate) fn fields(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
    ) -> Option<Vec<(u32, ValueShape, Option<semantic_vocabulary::ScalarType>)>> {
        let LegalizedScalarInstructionKind::EstablishRecord {
            result,
            fields,
            shape,
        } = &row.kind
        else {
            return None;
        };
        let declarations = &source.structural.as_ref()?.structural_types;
        if row.result.is_some()
            || crate::structural_inputs::shape(result.structural_type, declarations)? != *shape
        {
            return None;
        }
        let mut selected_declarations = declarations
            .iter()
            .filter(|entry| entry.id == result.structural_type);
        let declaration = selected_declarations.next()?;
        if selected_declarations.next().is_some() {
            return None;
        }
        let StructuralTypeShape::Record {
            fields: expected, ..
        } = &declaration.shape
        else {
            return None;
        };
        let expected = expected.iter().filter(|field| !field.relevance.is_erased());
        if fields.len() != expected.clone().count() {
            return None;
        }
        let mut offset = 0u32;
        let mut placed = Vec::with_capacity(fields.len());
        for (field, expected) in fields.iter().zip(expected) {
            if field.field != expected.id || expected.relevance.is_erased() {
                return None;
            }
            let field_shape = field_shape(source, result, field, expected)?;
            let alignment = u32::from(field_shape.alignment);
            offset = match expected.storage.as_ref() {
                Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(fragments)) => {
                    u32::try_from(fragments.byte_envelope()?.0).ok()?
                }
                Some(storage) => u32::try_from(storage.byte_offset()?).ok()?,
                None => offset.checked_add(alignment.checked_sub(1)?)? / alignment * alignment,
            };
            if offset.checked_add(field_shape.byte_size)? > shape.byte_size {
                return None;
            }
            placed.push((offset, field_shape, expected.field_type.scalar_type()));
            offset = offset.checked_add(field_shape.byte_size)?;
        }
        (offset <= shape.byte_size).then_some(placed)
    }

    pub(crate) fn field_shape(
        source: &LegalizedScalarFunction,
        parent: &terminal_psi::StructuralOperationResult,
        field: &terminal_psi::RecordFieldInitializer,
        expected: &terminal_psi::StructuralFieldDeclaration,
    ) -> Option<ValueShape> {
        if field.field != expected.id || expected.relevance.is_erased() {
            return None;
        }
        match &field.value {
            RecordFieldValue::Scalar {
                range_obligation, ..
            } => {
                if terminal_semantics::scalar_field_requires_evidence(&expected.field_type)
                    != range_obligation.is_some()
                {
                    return None;
                }
                let scalar = expected.field_type.scalar_type()?;
                match expected.storage.as_ref() {
                    Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(fragments)) => {
                        if expected.field_type.atomic_scalar_type().is_some() {
                            return None;
                        }
                        Some(ValueShape::integer(
                            u32::try_from(fragments.byte_envelope()?.1).ok()?,
                            1,
                        ))
                    }
                    Some(storage) => {
                        let codec = runtime_abi::ScalarStorageCodec::from_scalar(
                            storage.stored_scalar().unwrap_or(scalar),
                        )?;
                        Some(ValueShape::integer(u32::from(codec.byte_size()), 1))
                    }
                    None => crate::selection::scalar_call_abi::scalar_shape(scalar),
                }
            }
            RecordFieldValue::Structural(argument) => {
                structural_operand_shape(source, parent, argument, &expected.field_type)
            }
        }
    }

    fn place_parameters<'a>(
        source: &'a LegalizedScalarFunction,
        signature: &'a legalized_operations::LegalizedStructuralContract,
        place: semantic_vocabulary::PlaceId,
    ) -> impl Iterator<Item = &'a terminal_psi::StructuralParameterDeclaration> + 'a {
        signature
            .parameters
            .iter()
            .map(|parameter| &parameter.semantic)
            .chain(
                source
                    .blocks
                    .iter()
                    .filter(move |block| block.id != source.entry_block)
                    .flat_map(|block| &block.structural_parameters),
            )
            .filter(move |parameter| parameter.place == place)
    }

    pub(crate) fn structural_operand_shape(
        source: &LegalizedScalarFunction,
        parent: &terminal_psi::StructuralOperationResult,
        argument: &terminal_psi::StructuralArgument,
        field_type: &StructuralFieldType,
    ) -> Option<ValueShape> {
        let declarations = &source.structural.as_ref()?.structural_types;
        if !argument.path.is_empty() {
            let expected_type = field_type.as_structural()?;
            let (selected, _) = projected_operand(source, argument)?;
            if selected != expected_type || argument.place == parent.place {
                return None;
            }
            return crate::structural_inputs::structural_reference_input::shape(
                selected,
                declarations,
            );
        }
        let shape = match field_type {
            field_type
                if matches!(
                    field_type.byte_sequence_carrier(),
                    Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
                ) =>
            {
                if argument.access != terminal_psi::StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.place == parent.place
                {
                    return None;
                }
                let signature = source.structural.as_ref()?;
                let mut parameters = place_parameters(source, signature, argument.place);
                let child_type = if let Some(parameter) = parameters.next() {
                    if parameters.next().is_some()
                        || parameter.access != terminal_psi::StructuralAccess::Owned
                        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || !parameter.projected_qualifications.is_empty()
                    {
                        return None;
                    }
                    parameter.structural_type
                } else {
                    let (_, result) =
                        crate::selection::aggregate_result_input::home(source, argument.place)?;
                    if result.multiplicity != StructuralMultiplicity::Unrestricted
                        || !result.projected_qualifications.is_empty()
                        || !result.qualification_establishments.is_empty()
                        || !result.claims.is_empty()
                    {
                        return None;
                    }
                    result.structural_type
                };
                let declaration = find_structural_type(declarations, child_type)?;
                if declaration.shape
                    != StructuralTypeShape::ByteSequence(field_type.byte_sequence_carrier()?)
                {
                    return None;
                }
                crate::structural_inputs::structural_reference_input::shape(
                    child_type,
                    declarations,
                )?
            }
            field_type if descriptor_shape(field_type, declarations).is_some() => {
                let expected = descriptor_shape(field_type, declarations)?;
                if argument.access != terminal_psi::StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.place == parent.place
                {
                    return None;
                }
                let child_type = view_type(source, argument.place)
                    .or_else(|| element_view_type(source, argument.place))?;
                let readable = match view_access(source, argument.place) {
                    Some(terminal_psi::StructuralAccess::SharedBorrow) => true,
                    Some(terminal_psi::StructuralAccess::MutableBorrow) => {
                        parent.multiplicity == StructuralMultiplicity::Affine
                    }
                    _ => false,
                };
                if (find_structural_type(declarations, child_type)
                    .map(|declaration| &declaration.shape)
                    != Some(&expected)
                    && !descriptor_family(
                        find_structural_type(declarations, child_type)
                            .map(|declaration| &declaration.shape),
                        &expected,
                        view_access(source, argument.place),
                    ))
                    || !readable
                {
                    return None;
                }
                crate::structural_inputs::structural_reference_input::shape(
                    child_type,
                    declarations,
                )?
            }
            StructuralFieldType::Structural(nested) => {
                if argument.access != terminal_psi::StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.place == parent.place
                {
                    return None;
                }
                let signature = source.structural.as_ref()?;
                let mut matches = place_parameters(source, signature, argument.place)
                    .map(|parameter| {
                        (
                            parameter.structural_type,
                            parameter.access == terminal_psi::StructuralAccess::Owned,
                        )
                    })
                    .chain(source.instructions().filter_map(|row| {
                        let result = row.kind.get_structural_result()?;
                        (result.place == argument.place).then_some((result.structural_type, true))
                    }));
                let (child_type, plain_owned) = matches.next()?;
                if matches.next().is_some() || child_type != *nested || !plain_owned {
                    return None;
                }
                crate::structural_inputs::structural_reference_input::shape(*nested, declarations)?
            }
            _ => return None,
        };
        Some(shape)
    }

    fn descriptor_shape(
        field_type: &StructuralFieldType,
        declarations: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Option<StructuralTypeShape> {
        match field_type {
            StructuralFieldType::Structural(child) => find_structural_type(declarations, *child)
                .filter(|declaration| !declaration.owns_linear_claim)
                .map(|declaration| declaration.shape.clone())
                .filter(|shape| {
                    matches!(
                        shape,
                        StructuralTypeShape::ElementView { .. }
                            | StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                            )
                    )
                }),
            StructuralFieldType::ByteSequence(
                carrier @ terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
            )
            | StructuralFieldType::PredicateByteSequence {
                carrier: carrier @ terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                ..
            } => Some(StructuralTypeShape::ByteSequence(*carrier)),
            _ => None,
        }
    }

    fn descriptor_family(
        actual: Option<&StructuralTypeShape>,
        expected: &StructuralTypeShape,
        occurrence_access: Option<terminal_psi::StructuralAccess>,
    ) -> bool {
        match (actual, expected) {
            (
                Some(StructuralTypeShape::ElementView {
                    element: source,
                    access: source_access,
                }),
                StructuralTypeShape::ElementView {
                    element: destination,
                    access: destination_access,
                },
            ) => {
                source == destination
                    && destination_access.is_none_or(|requested| {
                        source_access.or(occurrence_access).is_some_and(|supplied| {
                            supplied == requested
                                || supplied == terminal_psi::StructuralAccess::MutableBorrow
                                    && matches!(
                                        requested,
                                        terminal_psi::StructuralAccess::SharedBorrow
                                            | terminal_psi::StructuralAccess::WriteOnlyBorrow
                                    )
                        })
                    })
            }
            (
                Some(StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                )),
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                ),
            ) => true,
            _ => false,
        }
    }

    pub(crate) fn projected_operand(
        source: &LegalizedScalarFunction,
        argument: &terminal_psi::StructuralArgument,
    ) -> Option<(semantic_vocabulary::StructuralTypeId, u32)> {
        if argument.access != terminal_psi::StructuralAccess::Owned || argument.path.is_empty() {
            return None;
        }
        let signature = source.structural.as_ref()?;
        let mut parameters = place_parameters(source, signature, argument.place);
        let root = if let Some(parameter) = parameters.next() {
            if parameters.next().is_some()
                || parameter.access != terminal_psi::StructuralAccess::Owned
            {
                return None;
            }
            parameter.structural_type
        } else {
            let (_, result) =
                crate::selection::aggregate_result_input::home(source, argument.place)?;
            result.structural_type
        };
        let (_, selected) = terminal_semantics::canonical_static_projection(
            signature.structural_types.iter(),
            root,
            &argument.path,
        )?;
        let (physical_type, offset) =
            crate::structural_inputs::structural_reference_input::project(
                root,
                &argument.path,
                &signature.structural_types,
            )?;
        (physical_type == selected.id).then_some((physical_type, offset))
    }
}

pub(crate) mod reference_input {
    use crate::structural_inputs::structural_reference_input::{
        PointerSlot, projected_scalar_codec, reference_formation_geometry, reference_projection,
        runtime_projection, scalar_storage_type,
    };
    use calling_conventions::ValueShape;
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use semantic_vocabulary::{PlaceId, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
    };
    use terminal_semantics::descriptor_results::{descriptor_value_access, is_descriptor};

    pub(crate) struct ReferenceAddress {
        pub(crate) place: PlaceId,
        pub(crate) byte_offset: u32,
        pub(crate) load_pointer: bool,
        pub(crate) load_codec: bool,
        pub(crate) codec: Option<runtime_abi::ScalarStorageCodec>,
        pub(crate) storage: Option<terminal_psi::ReferenceStorageView>,
        pub(crate) pointer_slots: Vec<PointerSlot>,
        pub(crate) indices: Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
    }

    pub(crate) struct SourceAccess {
        pub structural_type: StructuralTypeId,
        pub multiplicity: StructuralMultiplicity,
        pub access: StructuralAccess,
    }

    pub(crate) fn source_access(
        function: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<SourceAccess> {
        let signature = function.structural.as_ref()?;
        if let Some(parameter) =
            crate::selection::arrival_parameter_input::parameter(function, place)
        {
            let access = if parameter.access == StructuralAccess::Owned
                && is_descriptor(signature.structural_types.iter(), parameter.structural_type)
            {
                descriptor_value_access(
                    signature.structural_types.iter(),
                    parameter.structural_type,
                    parameter.multiplicity,
                )?
            } else {
                parameter.access
            };
            return Some(SourceAccess {
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access,
            });
        }
        if let Some((_, result)) =
            crate::selection::aggregate_result_input::operation_result(function, place)
        {
            let access = if is_descriptor(signature.structural_types.iter(), result.structural_type)
            {
                crate::selection::established_view_input::view_access(function, place)?
            } else {
                StructuralAccess::Owned
            };
            return Some(SourceAccess {
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                access,
            });
        }
        let semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } = signature.structural_place(place)?.kind
        else {
            return None;
        };
        Some(SourceAccess {
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: crate::selection::established_view_input::view_access(function, place)?,
        })
    }

    pub(crate) fn borrowed_scalar_source(
        function: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<semantic_vocabulary::ScalarType> {
        let signature = function.structural.as_ref()?;
        let parameter = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == place)
            .map(|parameter| &parameter.semantic)
            .or_else(|| {
                function
                    .block_structural_parameters()
                    .find(|parameter| parameter.place == place)
            })?;
        (parameter.access != StructuralAccess::Owned).then_some(())?;
        scalar_storage_type(parameter.structural_type, &signature.structural_types)
    }

    pub(crate) fn validate_scalar_foreign_export(
        function: &LegalizedScalarFunction,
        backings: &crate::selection::foreign_scalar_backings::ForeignScalarBackings<'_>,
        argument: &terminal_psi::StructuralArgument,
        scalar: semantic_vocabulary::ScalarType,
        width: u32,
    ) -> Result<(), crate::SelectedInstructionError> {
        if terminal_semantics::reference_storage_export_byte_width(scalar, argument.access)
            .map(u32::from)
            == Some(width)
            || backings.matches(function, argument, width)?
        {
            Ok(())
        } else {
            Err(crate::SelectedInstructionError::custody_because(
                "thin scalar foreign pointer requires the same byte footprint in every backing",
            ))
        }
    }

    pub(crate) fn establishment(
        function: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
    ) -> Option<ReferenceAddress> {
        let (result, source, storage, indices, shape, placed) = match &row.kind {
            LegalizedScalarInstructionKind::EstablishReference {
                result,
                source,
                storage,
                indices,
                shape,
            } => (result, source, *storage, indices, shape, None),
            LegalizedScalarInstructionKind::EstablishAccessReference {
                result,
                source,
                correspondence,
                indices,
                shape,
            } => (result, source, None, indices, shape, Some(correspondence)),
            _ => return None,
        };
        let signature = function.structural.as_ref()?;
        let declarations = &signature.structural_types;
        if row.result.is_some()
            || Some(*shape) != crate::structural_inputs::shape(result.structural_type, declarations)
            || result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
            || crate::selection::aggregate_result_input::home(function, result.place)
                != Some((row.operation, result))
        {
            return None;
        }
        let StructuralTypeShape::Reference { access, referent } = declarations
            .iter()
            .find(|declaration| declaration.id == result.structural_type)?
            .shape
        else {
            return None;
        };
        if source.access == StructuralAccess::Owned
            || !(access == source.access
                || source.access == StructuralAccess::MutableBorrow
                    && matches!(
                        access,
                        StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                    ))
        {
            return None;
        }
        let root = root_type(function, source.place)?;
        let owner = terminal_semantics::runtime_structural_path_tip(
            declarations.iter(),
            root,
            &source.path,
        )
        .map(|declaration| declaration.id);
        let address_source = match placed {
            Some(backing) => backing.address_source(source, declarations.as_slice())?,
            None => source.clone(),
        };
        let source = &address_source;
        let semantic_scalar = scalar_storage_type(referent, declarations);
        let (path, through_referent) = match source.path.split_last() {
            Some((StructuralPathSegment::Referent, prefix)) => (prefix, true),
            _ => (source.path.as_slice(), false),
        };
        let mut projection_root = root;
        let mut segment_start = 0;
        let geometry = reference_formation_geometry(root, path, declarations)?;
        if !crate::structural_inputs::runtime_indices_match(indices, &geometry.elements) {
            return None;
        }
        for (position, segment) in path.iter().enumerate() {
            if *segment != StructuralPathSegment::Referent {
                continue;
            }
            let (carrier, _, _) = runtime_projection(
                projection_root,
                &path[segment_start..position],
                declarations,
            )?;
            let StructuralTypeShape::Reference { referent, .. } =
                terminal_psi::find_structural_type(declarations, carrier)?.shape
            else {
                return None;
            };
            projection_root = referent;
            segment_start = position + 1;
        }
        let projection_path = &path[segment_start..];
        let byte_offset = geometry.byte_offset;
        let selected = match geometry.selected.as_ref() {
            terminal_psi::StructuralFieldType::Structural(identity) => Some(*identity),
            field => {
                let contract = if source.access == StructuralAccess::SharedBorrow {
                    field
                        .shared_observation_type()
                        .filter(|observed| observed.scalar_type().is_some())
                        .or_else(|| field.canonical_leaf_type())
                } else {
                    field.canonical_leaf_type()
                };
                contract.and_then(|contract| {
                    contract
                        .leaf_declaration(declarations.iter())
                        .map(|row| row.id)
                })
            }
        };
        let selected_shape = selected.and_then(|identity| {
            declarations
                .iter()
                .find(|row| row.id == identity)
                .map(|row| &row.shape)
        });
        let (actual, mut load_pointer) = match selected_shape {
            Some(StructuralTypeShape::Reference { referent, .. }) if through_referent => {
                (Some(*referent), true)
            }
            _ => (selected, false),
        };
        load_pointer |= borrowed_scalar_source(function, source.place).is_some();
        load_pointer |= placed.is_some_and(|backing| backing.resident_source().is_some());
        let load_codec = storage.is_none() && semantic_scalar.is_some() && load_pointer;
        let codec = match storage {
            Some(storage) => storage
                .stored_scalar
                .and_then(runtime_abi::ScalarStorageCodec::from_scalar),
            None if !load_codec && semantic_scalar.is_some() => {
                projected_scalar_codec(projection_root, projection_path, declarations)
            }
            None => None,
        };
        (if let Some(correspondence) = placed {
            let mut plans = declarations
                .access_views()
                .iter()
                .filter(|plan| plan.carrier == referent);
            match (plans.next(), plans.next()) {
                (Some(plan), None) => {
                    owner == Some(correspondence.source_type())
                        && (correspondence.resident_source().is_some()
                            || actual == Some(correspondence.schema()))
                        && optimization_unit_semantics::access_references::validate_backing(
                            declarations.as_slice(),
                            declarations.access_views(),
                            plan,
                            correspondence.source_type(),
                            correspondence,
                        )
                        .is_ok()
                }
                _ => false,
            }
        } else {
            match storage {
                Some(storage)
                    if matches!(
                        source.path.last(),
                        Some(StructuralPathSegment::FixedRange { .. })
                    ) =>
                {
                    terminal_semantics::reference_storage_region_source_compatible(
                        declarations.iter(),
                        root,
                        &source.path,
                        result.structural_type,
                        storage.stored_scalar,
                    )
                }
                Some(storage) => actual.is_some_and(|actual| {
                    terminal_semantics::reference_storage_source_compatible(
                        declarations.iter(),
                        actual,
                        result.structural_type,
                        storage.stored_scalar,
                    )
                }),
                None => terminal_semantics::reference_source_projection_compatible(
                    declarations.iter(),
                    root,
                    &source.path,
                    result.structural_type,
                ),
            }
        })
        .then_some(ReferenceAddress {
            place: source.place,
            byte_offset,
            load_pointer,
            load_codec,
            codec,
            storage,
            pointer_slots: geometry.pointer_slots,
            indices: indices.clone(),
        })
    }

    pub(crate) fn root_type(
        function: &LegalizedScalarFunction,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        let signature = function.structural.as_ref()?;
        let mut parameters = crate::selection::record_input::declared_parameters(function)?
            .filter(|parameter| parameter.place == place);
        if let Some(parameter) = parameters.next() {
            return parameters
                .next()
                .is_none()
                .then_some(parameter.structural_type);
        }
        if let Some((_, result, _, _)) =
            crate::selection::primitive_local_input::local(function, place)
        {
            return Some(result.structural_type);
        }
        crate::selection::aggregate_result_input::home(function, place)
            .map(|(_, result)| result.structural_type)
            .or_else(|| {
                let semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type,
                    ..
                } = signature.structural_place(place)?.kind
                else {
                    return None;
                };
                crate::selection::established_view_input::view_access(function, place)?;
                (crate::selection::established_view_input::view_type(function, place)
                    == Some(structural_type))
                .then_some(structural_type)
            })
    }

    pub(crate) fn referent_projection<'path>(
        function: &LegalizedScalarFunction,
        place: PlaceId,
        path: &'path [StructuralPathSegment],
        required_access: StructuralAccess,
    ) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
        let crossing = path
            .iter()
            .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
        let (structural_type, multiplicity) = if let Some((_, result)) =
            crate::selection::aggregate_result_input::home(function, place)
        {
            if !result.qualification_establishments.is_empty() || !result.claims.is_empty() {
                return None;
            }
            (result.structural_type, result.multiplicity)
        } else {
            let parameter = crate::selection::arrival_parameter_input::parameter(function, place)?;
            if function
                .structural
                .as_ref()?
                .entry_claims
                .iter()
                .any(|claim| claim.input == place)
            {
                return None;
            }
            (parameter.structural_type, parameter.multiplicity)
        };
        let source = source_access(function, place)?;
        if source.structural_type != structural_type || source.multiplicity != multiplicity {
            return None;
        }
        let declarations = &function.structural.as_ref()?.structural_types;
        if multiplicity == StructuralMultiplicity::Linear
            || !terminal_semantics::reference_source_projection_accessible(
                declarations.iter(),
                structural_type,
                multiplicity,
                source.access,
                path,
                required_access,
            )
        {
            return None;
        }
        let projection = reference_projection(
            structural_type,
            &path[..=crossing],
            required_access,
            None,
            declarations,
        )?;
        Some((projection.selected, &path[crossing + 1..]))
    }

    pub(crate) fn store_destination<'path>(
        function: &LegalizedScalarFunction,
        destination: &abstract_operations::StructuralScalarStoreDestination,
        path: &'path [StructuralPathSegment],
        required_access: StructuralAccess,
    ) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
        let admitted = match destination {
            abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                producer,
                result,
            } => {
                crate::selection::aggregate_result_input::home(function, result.place)
                    == Some((*producer, result))
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                block,
                parameter,
            } => {
                crate::selection::aggregate_result_input::block_home(function, parameter.place)
                    == Some((*block, parameter))
            }
            abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
                crate::selection::arrival_parameter_input::store_destination(function, parameter)
            }
        };
        if !admitted {
            return None;
        }
        referent_projection(function, destination.place(), path, required_access)
    }

    pub(crate) fn borrowed_argument(
        function: &LegalizedScalarFunction,
        argument: &terminal_psi::StructuralArgument,
        target: &target_operations::TargetStructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Option<BorrowedReferenceAddress> {
        let root = root_type(function, argument.place)?;
        let signature = function.structural.as_ref()?;
        let declarations = &signature.structural_types;
        match &target.source {
            target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
                let (producer, result) =
                    crate::selection::aggregate_result_input::home(function, argument.place)?;
                if producer != *psi_operation || result.structural_type != root {
                    return None;
                }
            }
            target_operations::TargetStructuralArgumentSource::Placement(placement) => {
                let parameter = signature
                    .parameters
                    .iter()
                    .find(|parameter| parameter.semantic.place == argument.place)?;
                if parameter.target.placement != *placement
                    || parameter.semantic.structural_type != root
                    || parameter.semantic.access == StructuralAccess::WriteOnlyBorrow
                {
                    return None;
                }
            }
            target_operations::TargetStructuralArgumentSource::BlockParameter { block, place } => {
                let (owner, parameter) =
                    crate::selection::aggregate_result_input::block_home(function, argument.place)?;
                if owner != *block || *place != argument.place || parameter.structural_type != root
                {
                    return None;
                }
            }
            _ => return None,
        }
        let projection = reference_projection(
            root,
            &argument.path,
            argument.access,
            Some(target.structural_type),
            declarations,
        )?;
        let selected = projection.selected;
        let offset = projection.referent_offset;
        let shape =
            crate::structural_inputs::structural_reference_input::shape(selected, declarations)?;
        (target.place == argument.place
            && target.path == argument.path
            && target.access == argument.access
            && target.root_structural_type == root
            && target.structural_type == selected
            && target.source_byte_offset == offset
            && target.shape
                == if scalar_storage_type(selected, declarations).is_some()
                    && target.destination.shape.class
                        == calling_conventions::ValueClass::BorrowedScalarReference
                {
                    ValueShape::borrowed_scalar_reference()
                } else {
                    ValueShape::borrowed_reference(shape.byte_size, shape.alignment)
                }
            && crate::structural_inputs::runtime_indices_match(indices, &projection.elements)
            && target.fixed_array_length.is_none()
            && target.element_stride.is_none())
        .then_some(BorrowedReferenceAddress {
            pointer_slots: projection.pointer_slots,
            indices: indices.to_vec(),
            referent_offset: offset,
            scalar_descriptor: projection.scalar_descriptor,
        })
    }

    pub(crate) struct BorrowedReferenceAddress {
        pub(crate) pointer_slots: Vec<PointerSlot>,
        pub(crate) indices: Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
        pub(crate) referent_offset: u32,
        pub(crate) scalar_descriptor: bool,
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use abstract_operations::{AccessReferenceBacking, ResidentReferenceBacking};
        use semantic_vocabulary::{
            IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, StructuralDomainId,
            StructuralFieldId, StructuralTypeId, ValueId,
        };
        use terminal_psi::{
            BindingRelevance, ResidentReferenceSource, StructuralAccess, StructuralArgument,
            StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
            StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
            TerminalIdentityIndex,
        };

        fn type_id(value: u64) -> StructuralTypeId {
            StructuralTypeId::new(value).unwrap()
        }

        fn field(identity: u32, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
            StructuralFieldDeclaration {
                inline_carrier: None,
                id: StructuralFieldId::new(u64::from(identity)).unwrap(),
                identity: TerminalIdentityIndex(identity),
                relevance: BindingRelevance::Relevant,
                field_type,
                storage: None,
            }
        }

        fn declaration(identity: u32, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
            StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id: type_id(u64::from(identity)),
                identity: TerminalIdentityIndex(identity),
                owns_linear_claim: identity == 1,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                shape,
            }
        }

        fn catalog() -> Vec<StructuralTypeDeclaration> {
            let length = StructuralFieldType::Scalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            ));
            vec![
                declaration(
                    1,
                    StructuralTypeShape::Record {
                        fields: vec![
                            field(10, length.clone()),
                            field(
                                11,
                                StructuralFieldType::Scalar(ScalarType::Integer(
                                    IntegerType::address(64).unwrap(),
                                )),
                            ),
                        ],
                        storage: StructuralRecordStorage::Packed,
                    },
                ),
                declaration(
                    2,
                    StructuralTypeShape::Reference {
                        referent: type_id(1),
                        access: StructuralAccess::MutableBorrow,
                    },
                ),
                declaration(
                    3,
                    StructuralTypeShape::Record {
                        fields: vec![
                            field(20, length),
                            field(21, StructuralFieldType::Structural(type_id(2))),
                        ],
                        storage: StructuralRecordStorage::Packed,
                    },
                ),
                declaration(
                    4,
                    StructuralTypeShape::FixedArray {
                        element: type_id(3),
                        length: terminal_psi::ArrayExtent::Literal(4),
                    },
                ),
            ]
        }

        fn backing() -> AccessReferenceBacking {
            AccessReferenceBacking::Resident(ResidentReferenceBacking {
                owner: type_id(1),
                schema: type_id(5),
                carrier: type_id(6),
                roles: ResidentReferenceSource {
                    base: StructuralFieldId::new(11).unwrap(),
                    length: StructuralFieldId::new(10).unwrap(),
                    granted: StructuralDomainId::new(1).unwrap(),
                    resident: StructuralDomainId::new(2).unwrap(),
                },
                placement_commitment: [7; 32],
            })
        }

        #[test]
        fn resident_address_uses_authenticated_field_without_replacing_owner() {
            let types = catalog();
            let source = StructuralArgument::whole(
                PlaceId::new(1).unwrap(),
                StructuralAccess::MutableBorrow,
            );
            let physical = backing().address_source(&source, &types).unwrap();
            assert!(source.path.is_empty());
            assert_eq!(physical.place, source.place);
            assert_eq!(physical.access, source.access);
            assert_eq!(
                physical.path,
                vec![StructuralPathSegment::Field(TerminalIdentityIndex(11))]
            );
            let geometry =
                crate::reference_formation_geometry(type_id(1), &physical.path, &types).unwrap();
            assert_eq!(geometry.byte_offset, 8);
            assert!(geometry.pointer_slots.is_empty());
            assert_eq!(
                geometry.selected.scalar_type(),
                Some(ScalarType::Integer(IntegerType::address(64).unwrap()))
            );
            terminal_semantics::validate_resident_owner_fields(
                &types[0],
                StructuralFieldId::new(10).unwrap(),
                StructuralFieldId::new(11).unwrap(),
            )
            .unwrap_err();
        }

        #[test]
        fn resident_address_retains_indices_before_pointer_crossings() {
            let types = catalog();
            let source = StructuralArgument {
                place: PlaceId::new(9).unwrap(),
                access: StructuralAccess::MutableBorrow,
                path: vec![
                    StructuralPathSegment::RuntimeIndex {
                        index: ValueId::new(3).unwrap(),
                        obligation: ObligationId::new(4).unwrap(),
                    },
                    StructuralPathSegment::Field(TerminalIdentityIndex(21)),
                    StructuralPathSegment::Referent,
                ],
            };
            let physical = backing().address_source(&source, &types).unwrap();
            assert_eq!(&physical.path[..source.path.len()], source.path.as_slice());
            assert_eq!(physical.place, source.place);
            let geometry =
                crate::reference_formation_geometry(type_id(4), &physical.path, &types).unwrap();
            assert_eq!(geometry.byte_offset, 8);
            assert_eq!(geometry.pointer_slots.len(), 1);
            assert_eq!(geometry.pointer_slots[0].byte_offset, 8);
            assert_eq!(geometry.pointer_slots[0].indices, 0..1);
            assert_eq!(geometry.elements.len(), 1);
            assert_eq!(geometry.elements[0].index, ValueId::new(3).unwrap());
            assert_eq!(geometry.elements[0].stride, 16);
            assert_eq!(geometry.elements[0].extent, Some(4));
        }
    }
}

pub(crate) mod scalar_array_input {
    use crate::selection::scalar_call_abi::scalar_shape;
    use calling_conventions::ValueShape;
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use semantic_vocabulary::{ScalarType, StructuralTypeId};

    pub(crate) fn storage<'a>(
        source: &LegalizedScalarFunction,
        row: &'a LegalizedScalarInstruction,
    ) -> Option<(
        &'a terminal_psi::StructuralOperationResult,
        ValueShape,
        Vec<(semantic_vocabulary::ValueId, ScalarType, u32, u8)>,
    )> {
        match &row.kind {
            LegalizedScalarInstructionKind::EstablishScalarArray {
                result,
                elements: values,
                shape,
            } => {
                let (scalar, width) = elements(source, row)?;
                let stores = values
                    .iter()
                    .enumerate()
                    .map(|(ordinal, value)| {
                        Some((
                            *value,
                            scalar,
                            u32::try_from(ordinal).ok()?.checked_mul(u32::from(width))?,
                            width,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some((result, *shape, stores))
            }
            _ => None,
        }
    }

    pub(crate) fn shape(
        source: &LegalizedScalarFunction,
        structural_type: StructuralTypeId,
    ) -> Option<(ScalarType, u64, ValueShape)> {
        let declarations = &source.structural.as_ref()?.structural_types;
        let mut current = structural_type;
        let mut count = Some(1u64);
        let mut empty = false;
        for depth in 0..=declarations.len() {
            let declaration = terminal_psi::find_structural_type(declarations, current)?;
            match declaration.shape {
                terminal_psi::StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    empty |= length == 0;
                    count = count.and_then(|count| count.checked_mul(length));
                    current = element;
                }
                terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) if depth != 0 => {
                    let leaf = scalar_shape(scalar)?;
                    let count = if empty { 0 } else { count? };
                    let bytes =
                        u32::try_from(count.checked_mul(u64::from(leaf.byte_size))?).ok()?;
                    return Some((
                        scalar,
                        count,
                        ValueShape::integer(bytes, if bytes == 0 { 1 } else { leaf.alignment }),
                    ));
                }
                _ => return None,
            }
        }
        None
    }

    pub(crate) fn elements(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
    ) -> Option<(ScalarType, u8)> {
        let LegalizedScalarInstructionKind::EstablishScalarArray {
            result,
            elements,
            shape: established,
        } = &row.kind
        else {
            return None;
        };
        let (scalar, count, shape) = shape(source, result.structural_type)?;
        if row.result.is_some()
            || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !result.claims.is_empty()
            || !result.projected_qualifications.is_empty()
            || *established != shape
            || u64::try_from(elements.len()).ok()? != count
        {
            return None;
        }
        Some((scalar, u8::try_from(scalar_shape(scalar)?.byte_size).ok()?))
    }
}

pub(crate) mod storage_origin_input {
    use crate::SelectedInstructionError;
    use crate::selection::reference_input::{establishment, root_type};
    use crate::structural_inputs::structural_reference_input::{
        descriptor_source_geometry, primitive_read_geometry, projected_scalar_codec,
        reference_formation_geometry, reference_source_geometry, runtime_indices_match,
        storage_address_geometry,
    };
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstructionKind as I,
        LegalizedScalarTerminator as T,
    };
    use selected_instructions::{
        SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole as R,
    };
    use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};
    use std::collections::{BTreeMap, BTreeSet};

    struct SourceGeometry<'a> {
        function: &'a LegalizedScalarFunction,
        selected_types: BTreeMap<
            (
                semantic_vocabulary::StructuralTypeId,
                Vec<terminal_psi::StructuralPathSegment>,
            ),
            semantic_vocabulary::StructuralTypeId,
        >,
    }

    #[derive(Default)]
    struct OriginInventory {
        coordinates: Vec<terminal_psi::StructuralArgument>,
        dependencies: Vec<(usize, usize)>,
        active: Vec<usize>,
        origins: Vec<usize>,
        storage_origins: Vec<usize>,
        unknown_storage: Vec<usize>,
        scalar_widths: Vec<(usize, u32)>,
        scalar_width_edges: BTreeSet<usize>,
    }

    impl OriginInventory {
        fn enter(&mut self, coordinate: &terminal_psi::StructuralArgument) -> bool {
            let previous = self.coordinates.iter().position(|previous| {
                terminal_semantics::same_descriptor_coordinate(previous, coordinate)
            });
            let index = previous.unwrap_or(self.coordinates.len());
            if let Some(parent) = self.active.last() {
                self.dependencies.push((*parent, index));
            }
            if previous.is_some() {
                return false;
            }
            self.coordinates.push(coordinate.clone());
            self.active.push(index);
            true
        }

        fn origin(&mut self) {
            if let Some(index) = self.active.last() {
                self.origins.push(*index);
                self.storage_origins.push(*index);
            }
        }

        fn returned_origin(&mut self, complete: bool) {
            if let Some(index) = self.active.last() {
                self.origins.push(*index);
            }
            if !complete && let Some(index) = self.active.last() {
                self.unknown_storage.push(*index);
            }
        }

        fn leave(&mut self) {
            self.active.pop();
        }

        fn validate(&self) -> Result<(), SelectedInstructionError> {
            let mut reached = vec![false; self.coordinates.len()];
            for index in &self.origins {
                reached[*index] = true;
            }
            loop {
                let mut changed = false;
                for (parent, child) in &self.dependencies {
                    if reached[*child] && !reached[*parent] {
                        reached[*parent] = true;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            if reached.iter().all(|reached| *reached) {
                Ok(())
            } else {
                Err(SelectedInstructionError::custody())
            }
        }
    }

    pub(crate) enum ScalarStorageOrigin {
        Width(u32),
        Source(terminal_psi::StructuralArgument),
    }
    pub(crate) fn scalar_storage_origins(
        function: &LegalizedScalarFunction,
        coordinate: &terminal_psi::StructuralArgument,
    ) -> Result<Option<Vec<ScalarStorageOrigin>>, SelectedInstructionError> {
        let mut geometry = SourceGeometry {
            function,
            selected_types: BTreeMap::new(),
        };
        let mut inventory = OriginInventory::default();
        collect_calls(
            &mut geometry,
            coordinate,
            &mut inventory,
            &mut BTreeSet::new(),
        )?;
        inventory.validate()?;
        let mut pending = vec![0];
        let mut seen = BTreeSet::new();
        let mut origins = Vec::new();
        while let Some(index) = pending.pop() {
            if !seen.insert(index) {
                continue;
            }
            if let Some((_, width)) = inventory
                .scalar_widths
                .iter()
                .find(|(node, _)| *node == index)
            {
                origins.push(ScalarStorageOrigin::Width(*width));
            }
            if inventory.unknown_storage.contains(&index) {
                return Ok(None);
            }
            if inventory.storage_origins.contains(&index) {
                origins.push(ScalarStorageOrigin::Source(
                    inventory.coordinates[index].clone(),
                ));
            }
            pending.extend(
                inventory
                    .dependencies
                    .iter()
                    .enumerate()
                    .filter(|(edge, (parent, _))| {
                        *parent == index && !inventory.scalar_width_edges.contains(edge)
                    })
                    .map(|(_, (_, child))| *child),
            );
        }
        if origins.is_empty() {
            Ok(None)
        } else {
            Ok(Some(origins))
        }
    }

    pub(crate) fn replay_memory_origins(
        source: &LegalizedScalarFunction,
        accesses: &mut Vec<SelectedMemoryAccess>,
    ) -> Result<(), SelectedInstructionError> {
        let mut geometry = SourceGeometry {
            function: source,
            selected_types: BTreeMap::new(),
        };
        let mut replayed = Vec::with_capacity(accesses.len());
        for access in accesses.iter() {
            if !matches!(
                access.role,
                R::ReadByteSequence { .. }
                    | R::ReadByteSpan { .. }
                    | R::ReadElementView { .. }
                    | R::ReadIndexedPrimitive { .. }
            ) {
                replayed.push(access.clone());
                continue;
            }
            let mut producers = BTreeSet::new();
            let mut visited = OriginInventory::default();
            for coordinate in access_coordinates(&mut geometry, access)? {
                collect_calls(&mut geometry, &coordinate, &mut visited, &mut producers)?;
            }
            visited.validate()?;
            if producers.is_empty() {
                replayed.push(access.clone());
            } else {
                for (producer, place) in producers {
                    let mut row = access.clone();
                    row.place = place;
                    row.byte_offset = 0;
                    row.byte_count = 0;
                    row.role = R::ReadReturnedView { producer };
                    replayed.push(row);
                }
            }
        }
        *accesses = replayed;
        Ok(())
    }

    fn borrowed_argument_sources(
        function: &LegalizedScalarFunction,
        place: PlaceId,
        arguments: &[&terminal_psi::StructuralArgument],
    ) -> Result<Vec<terminal_psi::StructuralArgument>, SelectedInstructionError> {
        let signature = function.structural.as_ref().ok_or_else(|| {
            SelectedInstructionError::custody_because(
                "borrowed descriptor operation lost its structural signature",
            )
        })?;
        let mut coordinates = Vec::new();
        for argument in arguments
            .iter()
            .copied()
            .filter(|argument| argument.place == place)
        {
            let root = root_type(function, argument.place).ok_or_else(|| {
                SelectedInstructionError::custody_because(
                    "borrowed descriptor operand lost its root type",
                )
            })?;
            let selected =
                storage_address_geometry(root, &argument.path, &signature.structural_types)
                    .ok_or_else(|| {
                        SelectedInstructionError::custody_because(
                            "borrowed descriptor operand lost its selected storage geometry",
                        )
                    })?;
            if selected.uses_borrowed_backing {
                coordinates.push(argument.clone());
            }
        }
        Ok(coordinates)
    }

    fn access_coordinates(
        geometry: &mut SourceGeometry<'_>,
        access: &SelectedMemoryAccess,
    ) -> Result<Vec<terminal_psi::StructuralArgument>, SelectedInstructionError> {
        let function = geometry.function;
        use terminal_psi::{StructuralAccess, StructuralArgument};
        let whole = |place| StructuralArgument::whole(place, StructuralAccess::SharedBorrow);
        let SelectedMemoryAccessOrigin::Operation(operation) = access.origin else {
            return Ok(vec![whole(access.place)]);
        };
        let row = function
            .instructions()
            .find(|row| row.operation == operation)
            .ok_or(SelectedInstructionError::custody())?;
        let coordinates = match &row.kind {
            I::ByteSequenceRead { source, .. } => vec![whole(*source)],
            I::ActivateTaskRegister {
                gdt,
                task_state,
                scratch,
                ..
            } => borrowed_argument_sources(function, access.place, &[gdt, task_state, scratch])?,
            I::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                borrowed_argument_sources(function, access.place, &[gdt, scratch])?
            }
            I::StorageAddress { source, path, .. }
            | I::GlobalDescriptorTable { source, path, .. } => {
                let signature = function
                    .structural
                    .as_ref()
                    .ok_or(SelectedInstructionError::custody())?;
                let root =
                    root_type(function, *source).ok_or(SelectedInstructionError::custody())?;
                let selected = storage_address_geometry(root, path, &signature.structural_types)
                    .ok_or(SelectedInstructionError::custody())?;
                if selected.uses_borrowed_backing {
                    vec![StructuralArgument {
                        place: *source,
                        path: path.clone(),
                        access: StructuralAccess::SharedBorrow,
                    }]
                } else {
                    Vec::new()
                }
            }
            I::PrimitiveScalarRead { source, path, .. } => {
                let signature = function
                    .structural
                    .as_ref()
                    .ok_or(SelectedInstructionError::custody())?;
                let root =
                    root_type(function, *source).ok_or(SelectedInstructionError::custody())?;
                let geometry = primitive_read_geometry(
                    root,
                    path,
                    row.result
                        .ok_or(SelectedInstructionError::custody())?
                        .scalar_type,
                    &signature.structural_types,
                )
                .ok_or(SelectedInstructionError::custody())?;
                if !geometry.uses_borrowed_backing {
                    Vec::new()
                } else {
                    vec![StructuralArgument {
                        place: *source,
                        path: path.clone(),
                        access: StructuralAccess::SharedBorrow,
                    }]
                }
            }
            I::ElementViewRead { source, path, .. } => {
                vec![StructuralArgument {
                    place: *source,
                    path: path.clone(),
                    access: StructuralAccess::SharedBorrow,
                }]
            }
            I::StructuralByteSequenceFieldRead { source, field, .. } => {
                vec![field_coordinate(geometry, source, Some(*field))?]
            }
            I::ByteSequenceLength { source, .. } | I::ElementViewLength { source, .. } => {
                vec![source.clone()]
            }
            I::ByteSequencePredicate { operand, .. } => vec![byte_coordinate(geometry, operand)?],
            I::ByteSequenceEqual { left, right }
            | I::EstablishByteSequenceConcatenation { left, right, .. } => {
                vec![
                    byte_coordinate(geometry, left)?,
                    byte_coordinate(geometry, right)?,
                ]
            }
            _ => vec![whole(access.place)],
        };
        Ok(coordinates)
    }

    fn byte_coordinate(
        geometry: &mut SourceGeometry<'_>,
        operand: &terminal_psi::control_flow::ByteSequenceOperand,
    ) -> Result<terminal_psi::StructuralArgument, SelectedInstructionError> {
        match operand {
            terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                Ok(source.clone())
            }
            terminal_psi::control_flow::ByteSequenceOperand::Field { source, field, .. } => {
                field_coordinate(geometry, source, Some(*field))
            }
        }
    }

    fn field_coordinate(
        geometry: &mut SourceGeometry<'_>,
        source: &terminal_psi::StructuralArgument,
        field: Option<semantic_vocabulary::StructuralFieldId>,
    ) -> Result<terminal_psi::StructuralArgument, SelectedInstructionError> {
        let function = geometry.function;
        let Some(field) = field else {
            return Ok(source.clone());
        };
        let signature = function
            .structural
            .as_ref()
            .ok_or(SelectedInstructionError::custody())?;
        let root = root_type(function, source.place).ok_or(SelectedInstructionError::custody())?;
        let (prefix, case) = match source.path.split_last() {
            Some((terminal_psi::StructuralPathSegment::Case(case), prefix)) => {
                (prefix, Some(*case))
            }
            _ => (source.path.as_slice(), None),
        };
        let key = (root, prefix.to_vec());
        let selected_type = match geometry.selected_types.get(&key) {
            Some(selected) => *selected,
            None => {
                let selected = reference_source_geometry(root, prefix, &signature.structural_types)
                    .ok_or(SelectedInstructionError::custody())?
                    .selected;
                geometry.selected_types.insert(key, selected);
                selected
            }
        };
        let shape = &signature
            .structural_type(selected_type)
            .ok_or(SelectedInstructionError::custody())?
            .shape;
        let fields = match case {
            Some(case) => shape
                .sum_cases()
                .and_then(|cases| cases.iter().find(|row| row.id == case))
                .map(|case| case.fields.as_slice()),
            None => shape.direct_fields(),
        }
        .ok_or(SelectedInstructionError::custody())?;
        let selected = terminal_psi::live_field_by_id(fields, field)
            .ok_or(SelectedInstructionError::custody())?;
        let mut coordinate = source.clone();
        coordinate
            .path
            .push(terminal_psi::StructuralPathSegment::Field(
                selected.identity,
            ));
        Ok(coordinate)
    }

    fn collect_calls(
        geometry: &mut SourceGeometry<'_>,
        coordinate: &terminal_psi::StructuralArgument,
        visited: &mut OriginInventory,
        calls: &mut BTreeSet<(OperationId, PlaceId)>,
    ) -> Result<(), SelectedInstructionError> {
        let function = geometry.function;
        if !visited.enter(coordinate) {
            return Ok(());
        }
        let place = coordinate.place;
        let signature = function
            .structural
            .as_ref()
            .ok_or(SelectedInstructionError::custody())?;
        let declaration = signature
            .structural_place(place)
            .ok_or(SelectedInstructionError::custody())?;
        let stored = stored_sources(geometry, coordinate, visited, calls)?;
        match declaration.kind {
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                let row = function
                    .instructions()
                    .find(|row| row.operation == producer)
                    .ok_or(SelectedInstructionError::custody())?;
                match &row.kind {
                    I::Call(call) => {
                        let (result, _) =
                            crate::selection::aggregate_result_input::call_result(function, call)
                                .ok_or(SelectedInstructionError::custody())?;
                        if result.place != place || result.structural_type != structural_type {
                            return Err(SelectedInstructionError::custody());
                        }
                        let inline_scalar = reference_formation_geometry(
                            structural_type,
                            &coordinate.path,
                            &signature.structural_types,
                        )
                        .is_some_and(|geometry| !geometry.uses_borrowed_backing)
                            && projected_scalar_codec(
                                structural_type,
                                &coordinate.path,
                                &signature.structural_types,
                            )
                            .is_some();
                        if inline_scalar {
                            visited.origin();
                        } else {
                            if call.descriptor_origins.is_empty() {
                                if contains_borrowed_storage(
                                    &signature.structural_types,
                                    structural_type,
                                    &mut BTreeSet::new(),
                                ) {
                                    return Err(SelectedInstructionError::custody());
                                }
                            } else {
                                calls.insert((producer, place));
                                if !coordinate.path.is_empty() {
                                    for origin in &call.descriptor_origins {
                                        if let legalized_operations::LegalizedDescriptorOrigin::Argument {
                                            source,
                                            ..
                                        } = origin
                                        {
                                            collect_calls(geometry, source, visited, calls)?;
                                        }
                                    }
                                }
                            }
                            visited.returned_origin(
                                !coordinate.path.is_empty()
                                    && !call.descriptor_origins.is_empty()
                                    && call.descriptor_origins.iter().all(|origin| {
                                        matches!(
                                            origin,
                                            legalized_operations::LegalizedDescriptorOrigin::Argument {
                                                exact: true,
                                                ..
                                            }
                                        )
                                    }),
                            );
                        }
                    }
                    I::ByteSequenceSubslice {
                        source: parent,
                        indices,
                        ..
                    }
                    | I::ElementViewSubslice {
                        source: parent,
                        indices,
                        ..
                    } => {
                        let root = root_type(function, parent.place)
                            .ok_or(SelectedInstructionError::custody())?;
                        let descriptor_geometry = descriptor_source_geometry(
                            root,
                            &parent.path,
                            &signature.structural_types,
                        )
                        .ok_or(SelectedInstructionError::custody())?;
                        if !descriptor_geometry.readable
                            || !runtime_indices_match(indices, &descriptor_geometry.elements)
                        {
                            return Err(SelectedInstructionError::custody());
                        }
                        collect_calls(geometry, parent, visited, calls)?;
                    }
                    I::StructuralLeafCopy {
                        source: parent,
                        path,
                        ..
                    } => {
                        let prefix = match path {
                            abstract_operations::StructuralCopyPath::Ordinary(path) => path.clone(),
                            abstract_operations::StructuralCopyPath::Case(path) => {
                                let root = root_type(function, *parent)
                                    .ok_or(SelectedInstructionError::custody())?;
                                let selected = terminal_semantics::descriptor_lifetimes::canonical_descriptor_location(
                                    &signature.structural_types, *parent, root, path,
                                ).ok_or(SelectedInstructionError::custody())?;
                                terminal_semantics::descriptor_lifetimes::descriptor_path(
                                    &signature.structural_types,
                                    root,
                                    &selected.path,
                                )
                                .ok_or(SelectedInstructionError::custody())?
                            }
                        };
                        let parent = terminal_psi::StructuralArgument {
                            place: *parent,
                            path: prefix,
                            access: coordinate.access,
                        };
                        collect_calls(
                            geometry,
                            &rebased(&parent, &coordinate.path),
                            visited,
                            calls,
                        )?;
                    }
                    I::EstablishElementView { source: parent, .. } => {
                        collect_calls(geometry, parent, visited, calls)?;
                    }
                    I::EstablishReference {
                        source: parent,
                        result,
                        ..
                    }
                    | I::EstablishAccessReference {
                        source: parent,
                        result,
                        ..
                    } => {
                        let known_width = if coordinate.path.is_empty()
                            || coordinate.path == [terminal_psi::StructuralPathSegment::Referent]
                        {
                            establishment(function, row)
                                .filter(|address| !address.load_codec)
                                .and_then(|address| address.codec)
                                .map(|codec| u32::from(codec.byte_size()))
                        } else {
                            None
                        };
                        let suffix = match coordinate.path.split_first() {
                            Some((terminal_psi::StructuralPathSegment::Referent, suffix)) => suffix,
                            None => &[],
                            _ => return Err(SelectedInstructionError::custody()),
                        };
                        let backing_edge = visited.dependencies.len();
                        let owner = *visited
                            .active
                            .last()
                            .ok_or_else(SelectedInstructionError::custody)?;
                        let backing = reference_backing_coordinate(
                            function,
                            result.structural_type,
                            parent,
                            suffix,
                        )?;
                        collect_calls(geometry, &backing, visited, calls)?;
                        if let Some(width) = known_width {
                            if visited
                                .dependencies
                                .get(backing_edge)
                                .is_none_or(|(parent, _)| *parent != owner)
                            {
                                return Err(SelectedInstructionError::custody());
                            }
                            visited.scalar_widths.push((owner, width));
                            visited.scalar_width_edges.insert(backing_edge);
                        }
                    }
                    I::EstablishRecord { fields, .. } => {
                        constructed_fields(
                            geometry,
                            structural_type,
                            None,
                            fields,
                            coordinate,
                            visited,
                            calls,
                        )?;
                    }
                    I::EstablishCase {
                        result_case,
                        fields,
                        ..
                    } => {
                        constructed_fields(
                            geometry,
                            structural_type,
                            Some(*result_case),
                            fields,
                            coordinate,
                            visited,
                            calls,
                        )?;
                    }
                    I::EstablishStructuralArray { elements, .. } => {
                        if elements.is_empty() {
                            visited.origin();
                        }
                        match coordinate.path.split_first() {
                            None => {
                                for element in elements {
                                    collect_calls(geometry, element, visited, calls)?;
                                }
                            }
                            Some((
                                terminal_psi::StructuralPathSegment::FixedIndex(index),
                                suffix,
                            )) => {
                                let element = elements
                                    .get(
                                        usize::try_from(*index)
                                            .map_err(|_| SelectedInstructionError::custody())?,
                                    )
                                    .ok_or(SelectedInstructionError::custody())?;
                                collect_calls(geometry, &rebased(element, suffix), visited, calls)?;
                            }
                            Some((
                                terminal_psi::StructuralPathSegment::RuntimeIndex { .. },
                                suffix,
                            )) => {
                                for element in elements {
                                    collect_calls(
                                        geometry,
                                        &rebased(element, suffix),
                                        visited,
                                        calls,
                                    )?;
                                }
                            }
                            _ => return Err(SelectedInstructionError::custody()),
                        }
                    }
                    I::EstablishZeroedStructural { .. } => visited.origin(),
                    I::EstablishStructuralLocal { .. } => {
                        if !stored
                            && contains_borrowed_storage(
                                &signature.structural_types,
                                structural_type,
                                &mut BTreeSet::new(),
                            )
                        {
                            return Err(SelectedInstructionError::custody());
                        }
                        if !contains_borrowed_storage(
                            &signature.structural_types,
                            structural_type,
                            &mut BTreeSet::new(),
                        ) {
                            visited.origin();
                        }
                    }
                    _ => {
                        if contains_borrowed_storage(
                            &signature.structural_types,
                            structural_type,
                            &mut BTreeSet::new(),
                        ) {
                            return Err(SelectedInstructionError::custody());
                        }
                        visited.origin();
                    }
                }
            }
            StructuralPlaceKind::BlockParameter { block, .. } => {
                let mut arrivals = 0;
                for owner in &function.blocks {
                    let mut follow = |successor: &legalized_operations::LegalizedScalarSuccessor| -> Result<(), SelectedInstructionError> {
                        if successor.target == block {
                            let binding = successor.structural_binding(place)
                                .ok_or(SelectedInstructionError::custody())?;
                            arrivals += 1;
                            collect_calls(geometry, &rebased(&binding.argument, &coordinate.path), visited, calls)?;
                        }
                        Ok(())
                    };
                    match &owner.terminator {
                        T::Jump { successor, .. } => follow(successor)?,
                        T::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => {
                            follow(when_true)?;
                            follow(when_false)?;
                        }
                        T::StructuralCase { cases, .. }
                            if cases.iter().any(|case| case.target == block) =>
                        {
                            return Err(SelectedInstructionError::custody());
                        }
                        _ => {}
                    }
                }
                if arrivals == 0 {
                    return Err(SelectedInstructionError::custody());
                }
            }
            StructuralPlaceKind::Parameter { .. }
            | StructuralPlaceKind::ByteSequenceLiteral { .. }
            | StructuralPlaceKind::TrivialAffineLocal { .. } => visited.origin(),
            _ => return Err(SelectedInstructionError::custody()),
        }
        visited.leave();
        Ok(())
    }

    fn reference_backing_coordinate(
        function: &LegalizedScalarFunction,
        reference: semantic_vocabulary::StructuralTypeId,
        parent: &terminal_psi::StructuralArgument,
        suffix: &[terminal_psi::StructuralPathSegment],
    ) -> Result<terminal_psi::StructuralArgument, SelectedInstructionError> {
        if suffix.is_empty() {
            return Ok(parent.clone());
        }
        let declarations = &function
            .structural
            .as_ref()
            .ok_or_else(SelectedInstructionError::custody)?
            .structural_types;
        let terminal_psi::StructuralTypeShape::Reference { referent, .. } =
            terminal_psi::find_structural_type(declarations, reference)
                .ok_or_else(SelectedInstructionError::custody)?
                .shape
        else {
            return Err(SelectedInstructionError::custody());
        };
        let root =
            root_type(function, parent.place).ok_or_else(SelectedInstructionError::custody)?;
        let source = reference_formation_geometry(root, &parent.path, declarations)
            .ok_or_else(SelectedInstructionError::custody)?;
        if matches!(source.selected.as_ref(), terminal_psi::StructuralFieldType::Structural(actual) if *actual == referent)
        {
            return Ok(rebased(parent, suffix));
        }
        let projection = reference_formation_geometry(referent, suffix, declarations)
            .ok_or_else(SelectedInstructionError::custody)?;
        if projection.uses_borrowed_backing {
            return Err(SelectedInstructionError::custody_because(
                "representation-view pointer crossing lacks an exact source descriptor coordinate",
            ));
        }
        Ok(parent.clone())
    }

    fn rebased(
        parent: &terminal_psi::StructuralArgument,
        suffix: &[terminal_psi::StructuralPathSegment],
    ) -> terminal_psi::StructuralArgument {
        let mut result = parent.clone();
        result.path.extend_from_slice(suffix);
        result
    }

    fn constructed_fields(
        geometry: &mut SourceGeometry<'_>,
        identity: semantic_vocabulary::StructuralTypeId,
        case: Option<semantic_vocabulary::StructuralCaseId>,
        fields: &[terminal_psi::RecordFieldInitializer],
        coordinate: &terminal_psi::StructuralArgument,
        visited: &mut OriginInventory,
        calls: &mut BTreeSet<(OperationId, PlaceId)>,
    ) -> Result<(), SelectedInstructionError> {
        let function = geometry.function;
        if coordinate.path.is_empty() {
            if fields.is_empty() {
                visited.origin();
            }
            for field in fields {
                if let Some(child) = field.value.as_structural() {
                    collect_calls(geometry, child, visited, calls)?;
                } else {
                    visited.origin();
                }
            }
            return Ok(());
        }
        let (selected_case, path) = match coordinate.path.split_first() {
            Some((terminal_psi::StructuralPathSegment::Case(selected), suffix)) => {
                (Some(*selected), suffix)
            }
            _ => (None, coordinate.path.as_slice()),
        };
        if selected_case.is_some() && selected_case != case {
            visited.origin();
            return Ok(());
        }
        let (terminal_psi::StructuralPathSegment::Field(name), suffix) = path
            .split_first()
            .ok_or(SelectedInstructionError::custody())?
        else {
            return Err(SelectedInstructionError::custody());
        };
        let shape = &function
            .structural
            .as_ref()
            .ok_or(SelectedInstructionError::custody())?
            .structural_type(identity)
            .ok_or(SelectedInstructionError::custody())?
            .shape;
        let declared = match selected_case {
            Some(case) => shape
                .sum_cases()
                .and_then(|cases| cases.iter().find(|row| row.id == case))
                .map(|case| case.fields.as_slice()),
            None => shape.direct_fields(),
        }
        .ok_or(SelectedInstructionError::custody())?;
        let field = terminal_psi::live_field_by_identity(declared, *name)
            .ok_or(SelectedInstructionError::custody())?;
        let mut initializers = fields
            .iter()
            .filter(|initializer| initializer.field == field.id);
        let initializer = initializers
            .next()
            .ok_or(SelectedInstructionError::custody())?;
        if initializers.next().is_some() {
            return Err(SelectedInstructionError::custody());
        }
        if let Some(child) = initializer.value.as_structural() {
            collect_calls(geometry, &rebased(child, suffix), visited, calls)?;
        } else {
            visited.origin();
        }
        Ok(())
    }

    fn contains_borrowed_storage(
        types: &[terminal_psi::StructuralTypeDeclaration],
        identity: semantic_vocabulary::StructuralTypeId,
        visited: &mut BTreeSet<semantic_vocabulary::StructuralTypeId>,
    ) -> bool {
        if !visited.insert(identity) {
            return false;
        }
        let Some(declaration) = terminal_psi::find_structural_type(types, identity) else {
            return true;
        };
        match &declaration.shape {
            terminal_psi::StructuralTypeShape::ElementView { .. }
            | terminal_psi::StructuralTypeShape::Reference { .. } => true,
            terminal_psi::StructuralTypeShape::ByteSequence(carrier) => !matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            ),
            terminal_psi::StructuralTypeShape::FixedArray { element, .. } => {
                contains_borrowed_storage(types, *element, visited)
            }
            shape => {
                shape
                    .direct_fields()
                    .into_iter()
                    .flatten()
                    .chain(
                        shape
                            .sum_cases()
                            .into_iter()
                            .flatten()
                            .flat_map(|case| &case.fields),
                    )
                    .any(|field| {
                        field.field_type.as_structural().is_some_and(|identity| {
                            contains_borrowed_storage(types, identity, visited)
                        }) || field
                            .field_type
                            .byte_sequence_carrier()
                            .is_some_and(|carrier| {
                                !matches!(
                                    carrier,
                                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                )
                            })
                    })
            }
        }
    }

    fn stored_sources(
        geometry: &mut SourceGeometry<'_>,
        coordinate: &terminal_psi::StructuralArgument,
        visited: &mut OriginInventory,
        calls: &mut BTreeSet<(OperationId, PlaceId)>,
    ) -> Result<bool, SelectedInstructionError> {
        let function = geometry.function;
        let mut found = false;
        for row in function.instructions() {
            let (destination, field, value) = match &row.kind {
                I::StoreStructuralField {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } => (
                    terminal_psi::StructuralArgument {
                        place: destination.place(),
                        path: path.clone(),
                        access: coordinate.access,
                    },
                    *field,
                    *value,
                ),
                I::StructuralByteViewFieldStore {
                    destination,
                    field,
                    source,
                    ..
                } => (destination.clone(), *field, *source),
                _ => continue,
            };
            if destination.place != coordinate.place {
                continue;
            }
            let target = field_coordinate(geometry, &destination, field)?.path;
            if !terminal_semantics::structural_paths_may_overlap(&target, &coordinate.path) {
                continue;
            }
            found = true;
            let suffix = if target.len() <= coordinate.path.len() {
                &coordinate.path[target.len()..]
            } else {
                &[]
            };
            let parent = terminal_psi::StructuralArgument::whole(value, coordinate.access);
            collect_calls(geometry, &rebased(&parent, suffix), visited, calls)?;
        }
        Ok(found)
    }
}

pub(crate) mod structural_array_input {
    use calling_conventions::ValueShape;
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use terminal_psi::{StructuralAccess, StructuralTypeShape};

    pub(crate) fn elements(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
    ) -> Option<(ValueShape, Vec<u32>)> {
        let LegalizedScalarInstructionKind::EstablishStructuralArray {
            result,
            elements,
            shape,
        } = &row.kind
        else {
            return None;
        };
        let signature = source.structural.as_ref()?;
        let types = &signature.structural_types;
        let declaration = types
            .iter()
            .find(|entry| entry.id == result.structural_type)?;
        let StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } = declaration.shape
        else {
            return None;
        };
        let geometry = crate::structural_inputs::structural_reference_input::shape;
        if row.result.is_some()
            || u64::try_from(elements.len()).ok()? != length
            || geometry(result.structural_type, types)? != *shape
        {
            return None;
        }
        let element_shape = geometry(element, types)?;
        let alignment = u32::from(element_shape.alignment);
        let stride = element_shape
            .byte_size
            .checked_add(alignment.checked_sub(1)?)?
            / alignment
            * alignment;
        let mut offsets = Vec::with_capacity(elements.len());
        for (ordinal, argument) in elements.iter().enumerate() {
            if argument.access != StructuralAccess::Owned || argument.place == result.place {
                return None;
            }
            if crate::selection::record_input::structural_operand_shape(
                source,
                result,
                argument,
                &terminal_psi::StructuralFieldType::Structural(element),
            )? != element_shape
            {
                return None;
            }
            offsets.push(u32::try_from(ordinal).ok()?.checked_mul(stride)?);
        }
        Some((element_shape, offsets))
    }
}

pub(crate) mod structural_local_input {
    use calling_conventions::ValueShape;
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };

    pub(crate) fn storage<'a>(
        source: &LegalizedScalarFunction,
        row: &'a LegalizedScalarInstruction,
    ) -> Result<(&'a terminal_psi::StructuralOperationResult, ValueShape), &'static str> {
        let LegalizedScalarInstructionKind::EstablishStructuralLocal { result, shape } = &row.kind
        else {
            return Err("structural local input has another operation kind");
        };
        let declarations = &source
            .structural
            .as_ref()
            .ok_or("structural local has no structural catalog")?
            .structural_types;
        terminal_semantics::validate_vacant_structural_local(declarations.iter(), result)?;
        if row.result.is_some() {
            return Err("structural local unexpectedly publishes a scalar result");
        }
        let declared_shape = crate::structural_inputs::shape(result.structural_type, declarations)
            .ok_or("structural local has no closed storage shape")?;
        if declared_shape != *shape {
            return Err("structural local differs from its declared storage shape");
        }
        Ok((result, *shape))
    }
}

pub(crate) mod zeroed_structural_input {
    use crate::structural_inputs::structural_reference_input::{
        byte_field_length, case_layout, primitive_geometry, runtime_projection, scalar_shape,
    };
    use calling_conventions::ValueShape;
    use legalized_operations::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use semantic_vocabulary::{
        IntegerSign, IntegerType, ScalarType, StructuralCaseId, StructuralFieldId,
    };
    use terminal_psi::{
        StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
        StructuralTypeShape, find_structural_type,
    };

    pub(crate) fn home<'a>(
        source: &LegalizedScalarFunction,
        row: &'a LegalizedScalarInstruction,
    ) -> Option<(&'a StructuralOperationResult, ValueShape)> {
        let LegalizedScalarInstructionKind::EstablishZeroedStructural { result, shape } = &row.kind
        else {
            return None;
        };
        let declarations = &source.structural.as_ref()?.structural_types;
        if row.result.is_some()
            || !matches!(
                result.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            )
            || !result.claims.is_empty()
            || crate::structural_inputs::shape(result.structural_type, declarations)? != *shape
            || !terminal_semantics::zeroed_structural_shape(
                declarations.iter(),
                result.structural_type,
            )
        {
            return None;
        }
        Some((result, *shape))
    }

    pub(crate) fn storage(
        source: &LegalizedScalarFunction,
        result: &StructuralOperationResult,
    ) -> Option<Vec<(u32, u8)>> {
        let declarations = &source.structural.as_ref()?.structural_types;
        let leaves = terminal_semantics::zeroed_structural_leaves(
            declarations.iter(),
            result.structural_type,
        )?;
        let mut stores = Vec::with_capacity(leaves.len());
        for leaf in leaves {
            let (offset, width, elements) = match &leaf {
                terminal_semantics::ZeroedStructuralLeaf::RecordField {
                    path,
                    field,
                    scalar_type,
                } => crate::structural_inputs::structural_reference_input::store(
                    result.structural_type,
                    path,
                    *field,
                    *scalar_type,
                    declarations,
                )?,
                terminal_semantics::ZeroedStructuralLeaf::EmptyView {
                    path,
                    field: Some(field),
                    carrier,
                } if carrier.byte_sequence_carrier().is_some() => {
                    let scalar_type = ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .ok()?,
                    );
                    let (offset, width) = byte_field_length(
                        result.structural_type,
                        path,
                        *field,
                        scalar_type,
                        declarations,
                    )?;
                    (offset, width, Vec::new())
                }
                // Typed immutable sequences retain real backing in Psi. Native
                // descriptor initialization still needs its ordinary layout owner.
                terminal_semantics::ZeroedStructuralLeaf::EmptyView { .. } => return None,
                terminal_semantics::ZeroedStructuralLeaf::CaseField {
                    path,
                    case,
                    field,
                    scalar_type,
                } => {
                    let (carrier, base, elements) =
                        runtime_projection(result.structural_type, path, declarations)?;
                    let carrier_declaration = find_structural_type(declarations, carrier)?;
                    let (field_offset, field_width) = case_payload_geometry(
                        carrier_declaration,
                        *case,
                        *field,
                        *scalar_type,
                        declarations,
                    )?;
                    (
                        base.checked_add(field_offset)?,
                        u32::from(field_width),
                        elements,
                    )
                }
                terminal_semantics::ZeroedStructuralLeaf::ArrayScalar { path, scalar_type } => {
                    primitive_geometry(result.structural_type, path, *scalar_type, declarations)?
                }
                terminal_semantics::ZeroedStructuralLeaf::EmptyScalarArray { .. } => {
                    continue;
                }
                terminal_semantics::ZeroedStructuralLeaf::ByteArray { path, length } => {
                    let byte =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).ok()?);
                    for index in (0..*length).rev() {
                        let mut element = path.clone();
                        element.push(StructuralPathSegment::FixedIndex(index));
                        let (offset, width, elements) = primitive_geometry(
                            result.structural_type,
                            &element,
                            byte,
                            declarations,
                        )?;
                        if !elements.is_empty() {
                            return None;
                        }
                        stores.push((offset, u8::try_from(width).ok()?));
                    }
                    continue;
                }
                terminal_semantics::ZeroedStructuralLeaf::SumSelection {
                    path,
                    structural_type,
                    case,
                    scalar_type,
                } => {
                    let (carrier, base, elements) =
                        runtime_projection(result.structural_type, path, declarations)?;
                    if carrier != *structural_type {
                        return None;
                    }
                    let carrier_declaration = find_structural_type(declarations, carrier)?;
                    let (StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. }) = &carrier_declaration.shape
                    else {
                        return None;
                    };
                    if cases.first().map(|first| first.id) != Some(*case) {
                        return None;
                    }
                    let layout = case_layout(carrier, declarations)?;
                    if scalar_shape(*scalar_type)? != layout.tag_shape {
                        return None;
                    }
                    (
                        base.checked_add(layout.tag_byte_offset)?,
                        layout.tag_shape.byte_size,
                        elements,
                    )
                }
            };
            if !elements.is_empty() {
                return None;
            }
            stores.push((offset, u8::try_from(width).ok()?));
        }
        Some(stores)
    }

    fn case_payload_geometry(
        carrier: &StructuralTypeDeclaration,
        case: StructuralCaseId,
        field: StructuralFieldId,
        scalar_type: ScalarType,
        declarations: &[StructuralTypeDeclaration],
    ) -> Option<(u32, u8)> {
        let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
            &carrier.shape
        else {
            return None;
        };
        let first = cases.first()?;
        if first.id != case {
            return None;
        }
        let (position, declaration) = first
            .fields
            .iter()
            .filter(|candidate| !candidate.relevance.is_erased())
            .enumerate()
            .find(|(_, candidate)| candidate.id == field)?;
        if declaration.field_type.scalar_type() != Some(scalar_type) {
            return None;
        }
        let packed = *case_layout(carrier.id, declarations)?
            .cases
            .first()?
            .fields
            .get(position)?;
        let shape = scalar_shape(scalar_type)?;
        if packed.shape != shape {
            return None;
        }
        Some((packed.byte_offset, u8::try_from(shape.byte_size).ok()?))
    }
}
