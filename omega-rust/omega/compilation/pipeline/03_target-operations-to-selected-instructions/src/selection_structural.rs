/*
We select place-backed structural operations in the ordinary instruction stream,
managing per-function `Transport` state and dispatching record, case, array,
local-storage, reference, runtime-address, activation-claim, and native-code-layout
operations alongside the view/byte (`selection_views_and_bytes`), access-lifecycle
(`selection_access_lifecycles`), and interrupt (`selection_interrupts`) modules.
*/

#[allow(unused_imports)]
pub(crate) use crate::SelectedInstructionError;
#[allow(unused_imports)]
pub(crate) use crate::selection::byte_view_homes::ByteViewHomes;
#[allow(unused_imports)]
pub(crate) use crate::selection::construction::scalar_graph::call_arguments::{
    invocation_memory_origin, invocation_provenance,
};
#[allow(unused_imports)]
pub(crate) use crate::selection::construction::scalar_graph::{
    Builder, IntegerSign, LegalizedScalarFunction, LegalizedScalarInstructionKind, ScalarType,
    SelectedBlockId, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
    ValueLocation, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
#[allow(unused_imports)]
pub(crate) use crate::selection::element_view_homes::ElementViewHomes;
#[allow(unused_imports)]
pub(crate) use calling_conventions::IndirectPointerLocation;
#[allow(unused_imports)]
pub(crate) use legalized_operations::LegalizedScalarInstruction;
#[allow(unused_imports)]
pub(crate) use optimization_core::AcceptedObligationFactIdentity;
#[allow(unused_imports)]
pub(crate) use selected_instructions::{
    SelectedBoundarySettlement, SelectedCallContract, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedOutgoingArgumentSlot,
};
#[allow(unused_imports)]
pub(crate) use semantic_vocabulary::{
    IntegerType, IntegerValue, ObligationId, OperationId, PlaceId, ValueId,
};
#[allow(unused_imports)]
pub(crate) use terminal_psi::StructuralAccess;

pub(crate) mod block_views {
    use super::{
        Builder, LegalizedScalarFunction, SelectedInstructionProvenance, SelectedMemoryAccessRole,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use selected_instructions::{
        LocalStorageSlotId, SelectedLocalStorageSlot, SelectedMemoryAccessOrigin,
    };

    pub(crate) fn block_entry(
        source: &LegalizedScalarFunction,
        block: &legalized_operations::LegalizedScalarBlock,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        for parameter in &block.structural_parameters {
            let joined =
                crate::selection::address_join_input::join(source, parameter.place).is_some();
            let shape = if joined {
                crate::selection::address_join_input::carrier_shape(source, parameter)
            } else {
                crate::selection::aggregate_result_input::block_parameter_shape(source, parameter)
                    .ok_or(SelectedInstructionError::custody())?
            };
            let slot = LocalStorageSlotId::StructuralBlockParameter {
                block: block.id,
                place: parameter.place,
            };
            builder
                .transport
                .local_slots
                .push(SelectedLocalStorageSlot {
                    id: slot,
                    byte_size: shape.byte_size.max(1),
                    alignment: shape.alignment,
                });
            let pointer = transport_register(builder, parameter.place, 0)?;
            super::memory_at_origin(
                builder,
                SelectedMemoryAccessOrigin::Block(block.id),
                parameter.place,
                0,
                shape.byte_size.max(1),
                SelectedMemoryAccessRole::AddressLocal { slot },
            )?;
            builder.emit_local_frame_address(
                slot,
                pointer,
                SelectedInstructionProvenance::default(),
            )?;
            let pointer = if joined && shape.byte_size == 8 {
                let referent = transport_register(builder, parameter.place, 0)?;
                super::memory_at_origin(
                    builder,
                    SelectedMemoryAccessOrigin::Block(block.id),
                    parameter.place,
                    0,
                    shape.byte_size,
                    SelectedMemoryAccessRole::ReadPlace,
                )?;
                builder.emit_load64(
                    0,
                    pointer,
                    referent,
                    SelectedInstructionProvenance::default(),
                )?;
                referent
            } else {
                pointer
            };
            builder.transport.pointers.push((parameter.place, pointer));
        }
        Ok(())
    }
}
pub(crate) mod entry {
    use super::{
        Builder, IndirectPointerLocation, IntegerSign, IntegerType, LegalizedScalarFunction,
        PlaceId, ScalarType, SelectedInstructionProvenance, StructuralAccess, ValueLocation,
        VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use crate::SelectedInstructionError;
    use crate::selection::aggregate_result_input::{
        inline_argument_fragments, parameter_home_required,
    };
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::structural_inputs::structural_unit_input::{
        accepts_graph, owned_indirect_pointer, parameter_views,
    };

    pub(crate) fn entry(
        function: usize,
        source: &LegalizedScalarFunction,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let Some(signature) = &source.structural else {
            return Ok(());
        };
        let parameters = parameter_views(signature);
        if !accepts_graph(&source.call_plan, &parameters, &signature.structural_types) {
            return Err(SelectedInstructionError::unsupported_shape(function));
        }
        for (parameter_index, parameter) in signature.parameters.iter().enumerate() {
            let place = parameter.semantic.place;
            let owned_pointer =
                owned_indirect_pointer(&parameter.semantic, &parameter.target.placement);
            if parameter.semantic.access == StructuralAccess::Owned
                && owned_pointer.is_none()
                && !inline_argument_fragments(&parameter.target.placement)
            {
                return Err(invalid());
            }
            if parameter.semantic.access == StructuralAccess::Owned
                && inline_argument_fragments(&parameter.target.placement)
            {
                if let Some(ValueLocation::Stack {
                    stack_byte_offset: base,
                    ..
                }) = parameter.target.placement.locations.first()
                {
                    let native_parameter = source
                        .parameters
                        .len()
                        .checked_add(parameter_index)
                        .ok_or_else(|| invalid())?;
                    if source.call_plan.parameters.get(native_parameter)
                        != Some(&parameter.target.placement)
                    {
                        return Err(invalid());
                    }
                    let address = transport_register(builder, place, 0)?;
                    builder.emit_frame_address(
                        selected_instructions::FrameStorageSlotId::Incoming {
                            parameter_index: native_parameter.try_into().map_err(|_| invalid())?,
                            abi_stack_byte_offset: *base,
                        },
                        address,
                        Default::default(),
                    )?;
                    for location in &parameter.target.placement.locations {
                        let ValueLocation::Stack {
                            stack_byte_offset,
                            value_byte_offset,
                            ..
                        } = location
                        else {
                            return Err(invalid());
                        };
                        let offset = *value_byte_offset;
                        if stack_byte_offset.checked_sub(*base) != Some(offset) {
                            return Err(invalid());
                        }
                    }
                    builder.transport.pointers.push((place, address));
                    retain_owned_home(source, parameter, builder)?;
                    continue;
                }
                for location in &parameter.target.placement.locations {
                    let ValueLocation::Register {
                        register,
                        value_byte_offset,
                        ..
                    } = location
                    else {
                        return Err(invalid());
                    };
                    let fixed = environment
                        .fixed_register_view(*register)
                        .ok_or_else(|| invalid())?;
                    let input = fixed_input_register(builder, place, parameter_index, fixed)?;
                    let offset = *value_byte_offset;
                    let output = transport_register(builder, place, offset)?;
                    builder.emit_copy_i64(
                        input,
                        output,
                        SelectedInstructionProvenance::default(),
                    )?;
                    builder.transport.fragments.push((place, offset, output));
                }
                retain_owned_home(source, parameter, builder)?;
                continue;
            }
            let used = crate::selection::parameter_use::referent_used(source, place);
            if owned_pointer.is_none() && !used {
                continue;
            }
            let stack_pointer_offset = match owned_pointer {
                Some(IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                }) => Some(stack_byte_offset),
                _ => crate::structural_inputs::structural_reference_input::stack_pointer_offset(
                    &parameter.target.placement,
                ),
            };
            if let Some(abi_stack_byte_offset) = stack_pointer_offset {
                let native_parameter = source
                    .parameters
                    .len()
                    .checked_add(parameter_index)
                    .ok_or_else(|| invalid())?;
                let address = transport_register(builder, place, 0)?;
                builder.emit_frame_address(
                    selected_instructions::FrameStorageSlotId::Incoming {
                        parameter_index: native_parameter.try_into().map_err(|_| invalid())?,
                        abi_stack_byte_offset,
                    },
                    address,
                    SelectedInstructionProvenance::default(),
                )?;
                let pointer = transport_register(builder, place, 0)?;
                builder.emit_load64(
                    0,
                    address,
                    pointer,
                    SelectedInstructionProvenance::default(),
                )?;
                builder.transport.pointers.push((place, pointer));
                if parameter.semantic.access == StructuralAccess::Owned {
                    retain_owned_home(source, parameter, builder)?;
                }
                continue;
            }
            let pointer = match parameter.target.placement.locations.as_slice() {
                [
                    ValueLocation::Indirect {
                        pointer: IndirectPointerLocation::Register(pointer),
                        ..
                    },
                ] => pointer,
                [
                    ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size: 8,
                    },
                ] if matches!(
                    parameter.semantic.access,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                ) =>
                {
                    register
                }
                _ => return Err(invalid()),
            };
            let fixed = environment
                .fixed_register_view(*pointer)
                .ok_or_else(|| invalid())?;
            let input = fixed_input_register(builder, place, parameter_index, fixed)?;
            let output = transport_register(builder, place, 0)?;
            builder.emit_copy_i64(input, output, SelectedInstructionProvenance::default())?;
            builder.transport.pointers.push((place, output));
            if parameter.semantic.access == StructuralAccess::Owned {
                retain_owned_home(source, parameter, builder)?;
            }
        }
        Ok(())
    }

    fn fixed_input_register(
        builder: &mut Builder<'_>,
        place: PlaceId,
        parameter_index: usize,
        fixed: register_model::RegisterViewId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let input = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
        builder.registers.push(VirtualRegister {
            id: input,
            scalar_type: ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            ),
            class: builder.class,
            origin: VirtualRegisterOrigin::StructuralParameter {
                place,
                parameter_index,
            },
            definition_site: None,
            entry_fixed_view: Some(fixed),
        });
        Ok(input)
    }

    fn retain_owned_home(
        source: &LegalizedScalarFunction,
        parameter: &legalized_operations::LegalizedCallUnitParameter,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let place = parameter.semantic.place;
        if !parameter_home_required(source, place) {
            return Ok(());
        }
        let shape = parameter.target.shape;
        let slot = selected_instructions::LocalStorageSlotId::StructuralParameter { place };
        builder
            .transport
            .local_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: shape.byte_size.max(1),
                alignment: shape.alignment,
            });
        let pointer = transport_register(builder, place, 0)?;
        crate::selection::construction::scalar_graph::structural_case::memory(
            builder,
            source.entry_block,
            place,
            0,
            shape.byte_size.max(1),
            selected_instructions::SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit_local_frame_address(slot, pointer, Default::default())?;
        if let Some(position) = builder
            .transport
            .pointers
            .iter()
            .position(|(stored, _)| *stored == place)
        {
            let incoming = builder.transport.pointers.remove(position).1;
            super::byte_copy::copy(
                builder,
                selected_instructions::SelectedMemoryAccessOrigin::Block(source.entry_block),
                Default::default(),
                super::byte_copy::CopyEnd {
                    place,
                    pointer: incoming,
                    byte_offset: 0,
                    elements: &[],
                },
                super::byte_copy::CopyEnd {
                    place,
                    pointer,
                    byte_offset: 0,
                    elements: &[],
                },
                shape.byte_size,
            )?;
            builder.transport.pointers.push((place, pointer));
            return Ok(());
        }
        for location in &parameter.target.placement.locations {
            let (offset, width) = match location {
                calling_conventions::register_or_stack_pair_pattern!(
                    value_byte_offset,
                    byte_size,
                    ..
                ) => ((*value_byte_offset), *byte_size),
                _ => return Err(invalid()),
            };
            let value = builder
                .transport
                .fragments
                .iter()
                .find(|(stored, byte_offset, _)| *stored == place && *byte_offset == offset)
                .map(|(_, _, value)| *value)
                .ok_or_else(|| invalid())?;
            store_home_range(builder, source, place, pointer, offset, width, value)?;
        }
        builder.transport.pointers.push((place, pointer));
        Ok(())
    }

    fn store_home_range(
        builder: &mut Builder<'_>,
        source: &LegalizedScalarFunction,
        place: semantic_vocabulary::PlaceId,
        home: VirtualRegisterId,
        offset: u32,
        width: u32,
        value: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        let (destination, displacement) = super::displaced_memory_pointer(
            builder,
            place,
            home,
            offset,
            width,
            Default::default(),
        )?;
        crate::selection::construction::scalar_graph::structural_case::memory(
            builder,
            source.entry_block,
            place,
            offset,
            width,
            selected_instructions::SelectedMemoryAccessRole::WritePlace,
        )?;
        crate::selection::construction::scalar_graph::aggregate_memory::store(
            builder,
            destination,
            value,
            displacement,
            width,
        )
    }
}
pub(crate) use crate::selection_access_lifecycles as access_lifecycles;
pub(crate) use block_views::block_entry;
pub(crate) use entry::entry;
pub(crate) mod atomic_outcome {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
        SelectedMemoryAccessRole, VirtualRegisterId, invalid, memory, provenance,
        transport_register,
    };
    use crate::SelectedInstructionError;

    pub(crate) fn establish(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
        observed: VirtualRegisterId,
        tag: VirtualRegisterId,
        scalar: ScalarType,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::AtomicAccess {
            outcome: Some((result, layout)),
            event,
            ..
        } = &row.kind
        else {
            return Err(invalid());
        };
        let custody = event.single_attempt_custody().ok_or_else(invalid)?;
        let ScalarType::Integer(integer) = scalar else {
            return Err(invalid());
        };
        let declaration = source
            .structural
            .as_ref()
            .and_then(|signature| {
                signature
                    .structural_types
                    .iter()
                    .find(|row| row.id == result.structural_type)
            })
            .ok_or_else(invalid)?;
        if target_operations::conventional_atomic_once_layout(declaration, integer, custody)
            .as_ref()
            != Some(layout)
        {
            return Err(invalid());
        }
        let normalization =
            crate::selection::scalar_call_abi::integer_carrier_normalization(scalar);
        let prior = if normalization == SelectedInstructionKind::CopyI64 {
            observed
        } else {
            let normalized = transport_register(builder, result.place, 0)?;
            builder.emit(
                normalization,
                builder.constraints.keys.copy_i64,
                &[observed, normalized],
                provenance(row),
            )?;
            normalized
        };
        let value = match custody.observation {
            language_core::atomic::AtomicCompareExchangeObservationAxis::Observing => prior,
            language_core::atomic::AtomicCompareExchangeObservationAxis::NonObserving => {
                let terminal_psi::AtomicAccessEvent::CompareExchangeOnce { replacement, .. } =
                    event
                else {
                    return Err(invalid());
                };
                let (_, proposed, _, proposed_type) =
                    builder.resolve(*replacement).ok_or_else(invalid)?;
                if proposed_type != scalar {
                    return Err(invalid());
                }
                let one = super::fragmented_scalar::constant(builder, row, result.place, 1)?;
                let zero = super::fragmented_scalar::constant(builder, row, result.place, 0)?;
                builder.emit(
                    SelectedInstructionKind::CompareI64,
                    builder.constraints.keys.compare_i64,
                    &[tag, one],
                    provenance(row),
                )?;
                let committed = transport_register(builder, result.place, 0)?;
                builder.emit(
                    SelectedInstructionKind::MaterializeBooleanEqual,
                    builder.constraints.keys.materialize_boolean,
                    &[committed],
                    provenance(row),
                )?;
                let mask = super::fragmented_scalar::binary(
                    builder,
                    row,
                    result.place,
                    SelectedInstructionKind::WrappingSubtractI64,
                    zero,
                    committed,
                )?;
                let difference = super::fragmented_scalar::binary(
                    builder,
                    row,
                    result.place,
                    SelectedInstructionKind::BitwiseXorI64,
                    prior,
                    proposed,
                )?;
                let selected = super::fragmented_scalar::binary(
                    builder,
                    row,
                    result.place,
                    SelectedInstructionKind::BitwiseAndI64,
                    difference,
                    mask,
                )?;
                super::fragmented_scalar::binary(
                    builder,
                    row,
                    result.place,
                    SelectedInstructionKind::BitwiseXorI64,
                    proposed,
                    selected,
                )?
            }
        };
        let pointer = super::record::initialize_storage(row, builder, result.place, layout.shape)?;
        let payload = &layout.cases[0].fields[0];
        if layout.cases[2].fields != layout.cases[0].fields
            || (custody.observation
                == language_core::atomic::AtomicCompareExchangeObservationAxis::NonObserving
                && layout.cases[1].fields != layout.cases[0].fields)
        {
            return Err(invalid());
        }
        memory(
            builder,
            row,
            result.place,
            payload.byte_offset,
            payload.shape.byte_size,
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(
            payload.byte_offset,
            payload.shape.byte_size as u8,
            pointer,
            value,
            provenance(row),
        )?;
        memory(
            builder,
            row,
            result.place,
            layout.tag_byte_offset,
            layout.tag_shape.byte_size,
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(
            layout.tag_byte_offset,
            layout.tag_shape.byte_size as u8,
            pointer,
            tag,
            provenance(row),
        )?;
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }
}
pub(crate) use crate::selection_views_and_bytes::byte_concatenation;
pub(crate) use crate::selection_views_and_bytes::byte_copy;
pub(crate) use crate::selection_views_and_bytes::byte_field_read;
pub(crate) use crate::selection_views_and_bytes::byte_field_store;
pub(crate) use crate::selection_views_and_bytes::byte_view_field_store;
pub(crate) use crate::selection_views_and_bytes::byte_views;
pub(crate) mod descriptor_operands {
    use super::runtime_address::{
        cross_references, footprint, projection_base, projection_origin, scale,
    };
    use super::{
        Builder, LegalizedScalarFunction, SelectedMemoryAccessRole, VirtualRegisterId, invalid,
        memory, place_pointer, provenance, transport_register,
    };
    use crate::SelectedInstructionError;
    use crate::structural_inputs::structural_reference_input::{
        self as geometry, ReferenceSourceGeometry,
    };
    use legalized_operations::LegalizedRuntimeIndexOperand;
    use terminal_psi::{StructuralArgument, StructuralTypeShape};

    pub(crate) fn source_geometry(
        function: &LegalizedScalarFunction,
        source: &StructuralArgument,
    ) -> Option<(ReferenceSourceGeometry, StructuralTypeShape)> {
        let signature = function.structural.as_ref()?;
        let root = crate::selection::reference_input::root_type(function, source.place)?;
        let geometry =
            geometry::descriptor_source_geometry(root, &source.path, &signature.structural_types)?;
        if !geometry.readable || source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
            return None;
        }
        let shape =
            terminal_psi::find_structural_type(&signature.structural_types, geometry.selected)?
                .shape
                .clone();
        Some((geometry, shape))
    }

    pub(crate) fn completed_view_words(
        function: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        source: &StructuralArgument,
        indices: &[LegalizedRuntimeIndexOperand],
    ) -> Result<Option<[VirtualRegisterId; 2]>, SelectedInstructionError> {
        if !source.path.is_empty() {
            return Ok(None);
        }
        let (geometry, shape) = source_geometry(function, source).ok_or_else(invalid)?;
        if !geometry::runtime_indices_match(indices, &geometry.elements) {
            return Err(invalid());
        }
        let length = match shape {
            StructuralTypeShape::ElementView { .. } => builder
                .transport
                .element_view(source.place)
                .map(|view| view.element_length),
            shape if shape.is_borrowed_byte_carrier() => builder
                .transport
                .view(source.place)
                .map(|view| view.byte_length),
            _ => None,
        };
        let Some(length) = length else {
            return Ok(None);
        };
        let backing =
            projection_base(function, builder, row, source.place, 0)?.ok_or_else(invalid)?;
        Ok(Some([backing, length]))
    }

    pub(crate) fn backing(
        function: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        source: &StructuralArgument,
        indices: &[LegalizedRuntimeIndexOperand],
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let (geometry, shape) = source_geometry(function, source).ok_or(invalid())?;
        if !geometry::runtime_indices_match(indices, &geometry.elements) {
            return Err(invalid());
        }
        if let Some([backing, _]) = completed_view_words(function, builder, row, source, indices)? {
            return Ok(backing);
        }
        let (pointer, leaf_index) =
            stored_address(function, builder, row, source, indices, &geometry)?;
        let leaf_indices = &indices[leaf_index..];
        let address_width = if matches!(shape, StructuralTypeShape::FixedArray { .. }) {
            0
        } else {
            8
        };
        let (pointer, displacement) = super::memory_pointer(
            builder,
            row,
            source.place,
            pointer,
            geometry.byte_offset,
            address_width,
        )?;
        let output = transport_register(builder, source.place, geometry.byte_offset)?;
        match shape {
            StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                | terminal_psi::ByteSequenceCarrier::BoundedView { .. },
            ) => {
                footprint(
                    builder,
                    row,
                    source.place,
                    geometry.byte_offset,
                    8,
                    leaf_indices,
                    super::runtime_address::Written::None,
                )?;
                memory(
                    builder,
                    row,
                    source.place,
                    geometry.byte_offset,
                    8,
                    SelectedMemoryAccessRole::ReadPlace,
                )?;
                builder.emit_load64(displacement, pointer, output, provenance(row))?;
            }
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            ) => {
                builder.emit_address_offset(
                    displacement.checked_add(8).ok_or(invalid())?,
                    pointer,
                    output,
                    provenance(row),
                )?;
            }
            StructuralTypeShape::FixedArray { .. }
                if function.structural.as_ref().is_some_and(|signature| {
                    terminal_semantics::fixed_byte_extent(
                        signature.structural_types.iter(),
                        geometry.selected,
                    )
                    .is_some()
                }) =>
            {
                builder.emit_address_offset(displacement, pointer, output, provenance(row))?;
            }
            _ => return Err(invalid()),
        }
        Ok(output)
    }

    pub(crate) fn element_stride(
        function: &LegalizedScalarFunction,
        source: &StructuralArgument,
    ) -> Option<u32> {
        let (_, StructuralTypeShape::ElementView { element, .. }) =
            source_geometry(function, source)?
        else {
            return None;
        };
        let signature = function.structural.as_ref()?;
        let shape = geometry::shape(element, &signature.structural_types)?;
        geometry::align(shape.byte_size, shape.alignment).filter(|stride| *stride != 0)
    }

    fn stored_address(
        function: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        source: &StructuralArgument,
        indices: &[LegalizedRuntimeIndexOperand],
        geometry: &ReferenceSourceGeometry,
    ) -> Result<(VirtualRegisterId, usize), SelectedInstructionError> {
        let (pointer, pointer_slots) = if source.path.is_empty() {
            (
                Some(place_pointer(builder, row, source.place, 0)?),
                geometry.pointer_slots.as_slice(),
            )
        } else {
            projection_origin(
                function,
                builder,
                row,
                source.place,
                geometry.byte_offset,
                &geometry.pointer_slots,
            )?
        };
        let pointer = pointer.ok_or_else(invalid)?;
        let (pointer, leaf_index) =
            cross_references(builder, row, source.place, pointer, indices, pointer_slots)?;
        let leaf_indices = &indices[leaf_index..];
        let pointer = scale(
            builder,
            row,
            source.place,
            geometry.byte_offset,
            pointer,
            leaf_indices,
        )?;
        Ok((pointer, leaf_index))
    }

    pub(crate) fn view_words(
        function: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        source: &StructuralArgument,
        indices: &[LegalizedRuntimeIndexOperand],
    ) -> Result<[VirtualRegisterId; 2], SelectedInstructionError> {
        if let Some(words) = completed_view_words(function, builder, row, source, indices)? {
            return Ok(words);
        }
        let (geometry, shape) = source_geometry(function, source).ok_or_else(invalid)?;
        if !geometry::runtime_indices_match(indices, &geometry.elements)
            || !matches!(
                shape,
                StructuralTypeShape::ElementView { .. }
                    | StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    )
            )
        {
            return Err(invalid());
        }
        let (pointer, leaf) = stored_address(function, builder, row, source, indices, &geometry)?;
        let (pointer, displacement) = super::memory_pointer(
            builder,
            row,
            source.place,
            pointer,
            geometry.byte_offset,
            16,
        )?;
        footprint(
            builder,
            row,
            source.place,
            geometry.byte_offset,
            16,
            &indices[leaf..],
            super::runtime_address::Written::None,
        )?;
        let backing = transport_register(builder, source.place, geometry.byte_offset)?;
        let length_offset = geometry.byte_offset.checked_add(8).ok_or_else(invalid)?;
        let length = transport_register(builder, source.place, length_offset)?;
        for (offset, output) in [(0u32, backing), (8, length)] {
            memory(
                builder,
                row,
                source.place,
                geometry
                    .byte_offset
                    .checked_add(offset)
                    .ok_or_else(invalid)?,
                8,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            builder.emit_load64(
                displacement.checked_add(offset).ok_or_else(invalid)?,
                pointer,
                output,
                provenance(row),
            )?;
        }
        Ok([backing, length])
    }
}
pub(crate) use crate::selection_interrupts::interrupt_installation;
pub(crate) use crate::selection_interrupts::interrupt_installation_calls;
pub(crate) use crate::selection_interrupts::interrupt_installation_guards;
pub(crate) mod leaf_copy {
    use super::runtime_address::{cross_references, footprint, projection_origin, scale};
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind,
    };
    use crate::SelectedInstructionError;
    use crate::selection::aggregate_result_input::{block_home, inline_argument_fragments};
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::local_storage;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::structural_inputs::structural_reference_input::{
        case_projection, leaf_copy_geometry, reference_formation_geometry,
        reference_source_geometry,
    };
    use selected_instructions::VirtualRegisterId;

    pub(crate) fn copy(
        function: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::StructuralLeafCopy {
            result,
            source,
            byte_offset,
            shape,
            indices,
            path,
            ..
        } = &row.kind
        else {
            return Err(invalid());
        };
        let slot = builder.transport.push_structural_local_slot(
            row.operation,
            result.place,
            shape.byte_size.max(1),
            shape.alignment,
        );
        let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size.max(1), true)?;
        if shape.byte_size == 0 {
            builder.transport.pointers.push((result.place, pointer));
            return Ok(());
        }
        let input = builder
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == *source)
            .map(|(_, pointer)| *pointer);
        let geometry = match path {
            abstract_operations::StructuralCopyPath::Ordinary(path) if !path.is_empty() => {
                let signature = function.structural.as_ref().ok_or(invalid())?;
                let root = crate::selection::reference_input::root_type(function, *source)
                    .ok_or(invalid())?;
                Some(
                    leaf_copy_geometry(
                        root,
                        path,
                        result.structural_type,
                        &signature.structural_types,
                    )
                    .filter(|geometry| geometry.readable)
                    .ok_or(invalid())?,
                )
            }
            abstract_operations::StructuralCopyPath::Case(path) => {
                let signature = function.structural.as_ref().ok_or(invalid())?;
                let root = crate::selection::reference_input::root_type(function, *source)
                    .ok_or(invalid())?;
                Some(case_projection(root, path, &signature.structural_types).ok_or(invalid())?)
            }
            _ => None,
        };
        let pointer_slots = geometry
            .as_ref()
            .map_or(&[][..], |geometry| geometry.pointer_slots.as_slice());
        let (input, pointer_slots) = if geometry.is_some() {
            projection_origin(function, builder, row, *source, *byte_offset, pointer_slots)?
        } else {
            (input, pointer_slots)
        };
        let Some(input) = input else {
            copy_from_fragments(
                function,
                row,
                source,
                CopyEnd {
                    place: result.place,
                    pointer,
                    byte_offset: 0,
                    elements: &[],
                },
                *byte_offset,
                *shape,
                indices,
                builder,
            )?;
            builder.transport.pointers.push((result.place, pointer));
            return Ok(());
        };
        let (input, leaf_index) =
            cross_references(builder, row, *source, input, indices, pointer_slots)?;
        let indices = indices.get(leaf_index..).ok_or(invalid())?;
        let input = scale(builder, row, *source, *byte_offset, input, indices)?;
        copy_bytes(
            builder,
            row,
            CopyEnd {
                place: *source,
                pointer: input,
                byte_offset: *byte_offset,
                elements: indices,
            },
            CopyEnd {
                place: result.place,
                pointer,
                byte_offset: 0,
                elements: &[],
            },
            shape.byte_size,
        )?;
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }

    pub(crate) fn reseat(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::StoreStructuralField {
            destination,
            path,
            field,
            value,
            byte_offset,
            shape,
            indices,
            ..
        } = &row.kind
        else {
            return Err(invalid());
        };
        let destination_admits = match destination {
            abstract_operations::StructuralWindowRoot::Parameter(destination) => {
                matches!(
                    destination.access,
                    terminal_psi::StructuralAccess::Owned
                        | terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                ) && (source.structural.as_ref().is_some_and(|signature| {
                    signature
                        .parameters
                        .iter()
                        .any(|parameter| parameter.semantic == *destination)
                }) || source
                    .blocks
                    .iter()
                    .any(|block| block.structural_parameters.contains(destination)))
            }
            abstract_operations::StructuralWindowRoot::OwnedResult { .. } => true,
            abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
                block_home(source, parameter.place) == Some((*block, parameter))
            }
        };
        if row.result.is_some() || !destination_admits {
            return Err(invalid());
        }
        if shape.byte_size == 0 {
            builder.pending_provenance.operations.push(row.operation);
            builder
                .pending_provenance
                .work
                .extend(row.work.iter().cloned());
            return Ok(());
        }
        let types = &source
            .structural
            .as_ref()
            .ok_or(invalid())?
            .structural_types;
        let projection = terminal_semantics::structural_window_projection(
            types.iter(),
            destination.structural_type(),
            path,
            *field,
        )
        .ok_or(invalid())?;
        let geometry =
            reference_source_geometry(destination.structural_type(), &projection.path, types)
                .ok_or(invalid())?;
        if geometry.byte_offset != *byte_offset
            || !crate::structural_inputs::runtime_indices_match(indices, &geometry.elements)
        {
            return Err(invalid());
        }
        let (root, pointer_slots) = projection_origin(
            source,
            builder,
            row,
            destination.place(),
            *byte_offset,
            &geometry.pointer_slots,
        )?;
        let (root, leaf_index) = cross_references(
            builder,
            row,
            destination.place(),
            root.ok_or_else(invalid)?,
            indices,
            pointer_slots,
        )?;
        let indices = indices.get(leaf_index..).ok_or_else(invalid)?;
        let root = scale(
            builder,
            row,
            destination.place(),
            *byte_offset,
            root,
            indices,
        )?;
        let destination = CopyEnd {
            place: destination.place(),
            pointer: root,
            byte_offset: *byte_offset,
            elements: indices,
        };
        let home = builder
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == *value)
            .map(|(_, pointer)| *pointer);
        let Some(home) = home else {
            return copy_from_fragments(source, row, value, destination, 0, *shape, &[], builder);
        };
        copy_bytes(
            builder,
            row,
            CopyEnd {
                place: *value,
                pointer: home,
                byte_offset: 0,
                elements: &[],
            },
            destination,
            shape.byte_size,
        )
    }

    pub(crate) use super::byte_copy::CopyEnd;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn copy_operand(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
        argument: &terminal_psi::StructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
        destination: CopyEnd<'_>,
        shape: calling_conventions::ValueShape,
    ) -> Result<(), SelectedInstructionError> {
        if argument.access != terminal_psi::StructuralAccess::Owned {
            return Err(SelectedInstructionError::custody_because(
                "aggregate operand copy requires owned input",
            ));
        }
        if shape.byte_size == 0 {
            return Ok(());
        }
        let projected = project_operand(source, row, builder, argument, indices)?;
        let Some(input) = projected.pointer else {
            return copy_from_fragments(
                source,
                row,
                &argument.place,
                destination,
                projected.byte_offset,
                shape,
                projected.indices,
                builder,
            );
        };
        copy_bytes(
            builder,
            row,
            CopyEnd {
                place: argument.place,
                pointer: input,
                byte_offset: projected.byte_offset,
                elements: projected.indices,
            },
            destination,
            shape.byte_size,
        )
    }

    pub(crate) struct ProjectedOperand<'indices> {
        pub pointer: Option<VirtualRegisterId>,
        pub byte_offset: u32,
        pub indices: &'indices [legalized_operations::LegalizedRuntimeIndexOperand],
    }

    pub(crate) fn project_operand<'indices>(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
        argument: &terminal_psi::StructuralArgument,
        indices: &'indices [legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Result<ProjectedOperand<'indices>, SelectedInstructionError> {
        let signature = source.structural.as_ref().ok_or_else(invalid)?;
        let root = crate::selection::reference_input::root_type(source, argument.place)
            .ok_or_else(invalid)?;
        let geometry =
            reference_formation_geometry(root, &argument.path, &signature.structural_types)
                .ok_or_else(invalid)?;
        if !geometry.readable
            || !crate::structural_inputs::runtime_indices_match(indices, &geometry.elements)
        {
            return Err(SelectedInstructionError::custody_because(
                "aggregate operand lost its readable projection or selectors",
            ));
        }
        let (input, pointer_slots) = if argument.path.is_empty() {
            (
                builder
                    .transport
                    .pointers
                    .iter()
                    .find(|(place, _)| *place == argument.place)
                    .map(|(_, pointer)| *pointer),
                &[][..],
            )
        } else {
            projection_origin(
                source,
                builder,
                row,
                argument.place,
                geometry.byte_offset,
                &geometry.pointer_slots,
            )?
        };
        let Some(input) = input else {
            return Ok(ProjectedOperand {
                pointer: None,
                byte_offset: geometry.byte_offset,
                indices,
            });
        };
        let (input, consumed) =
            cross_references(builder, row, argument.place, input, indices, pointer_slots)?;
        let indices = indices.get(consumed..).ok_or_else(invalid)?;
        let input = scale(
            builder,
            row,
            argument.place,
            geometry.byte_offset,
            input,
            indices,
        )?;
        Ok(ProjectedOperand {
            pointer: Some(input),
            byte_offset: geometry.byte_offset,
            indices,
        })
    }

    fn copy_bytes(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        from: CopyEnd<'_>,
        to: CopyEnd<'_>,
        byte_size: u32,
    ) -> Result<(), SelectedInstructionError> {
        super::byte_copy::copy(
            builder,
            crate::selection::construction::scalar_graph::call_arguments::invocation_memory_origin(
                row,
            ),
            provenance(row),
            from,
            to,
            byte_size,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn copy_from_fragments(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        place: &semantic_vocabulary::PlaceId,
        destination: CopyEnd<'_>,
        byte_offset: u32,
        shape: calling_conventions::ValueShape,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        if !indices.is_empty() {
            return Err(invalid());
        }
        let parameter = source
            .structural
            .as_ref()
            .and_then(|signature| {
                signature
                    .parameters
                    .iter()
                    .find(|parameter| parameter.semantic.place == *place)
            })
            .ok_or_else(|| invalid())?;
        if !inline_argument_fragments(&parameter.target.placement) {
            return Err(invalid());
        }
        let leaf_end = byte_offset
            .checked_add(shape.byte_size)
            .ok_or_else(|| invalid())?;
        for location in &parameter.target.placement.locations {
            let (fragment_offset, width) = match location {
                calling_conventions::ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                }
                | calling_conventions::ValueLocation::Stack {
                    value_byte_offset,
                    byte_size,
                    ..
                } => ((*value_byte_offset), *byte_size),
                _ => return Err(invalid()),
            };
            let fragment_end = fragment_offset
                .checked_add(width)
                .ok_or_else(|| invalid())?;
            if fragment_end <= byte_offset || fragment_offset >= leaf_end {
                continue;
            }
            if fragment_offset < byte_offset || fragment_end > leaf_end {
                return Err(invalid());
            }
            let value = builder
                .transport
                .fragments
                .iter()
                .find(|(stored, offset, _)| *stored == *place && *offset == fragment_offset)
                .map(|(_, _, value)| *value)
                .ok_or_else(|| invalid())?;
            let offset = destination
                .byte_offset
                .checked_add(fragment_offset - byte_offset)
                .ok_or_else(|| invalid())?;
            let (pointer, displacement) = super::memory_pointer(
                builder,
                row,
                destination.place,
                destination.pointer,
                offset,
                width,
            )?;
            footprint(
                builder,
                row,
                destination.place,
                offset,
                width,
                destination.elements,
                super::runtime_address::Written::Structural,
            )?;
            builder.pending_provenance.operations.push(row.operation);
            crate::selection::construction::scalar_graph::aggregate_memory::store(
                builder,
                pointer,
                value,
                displacement,
                width,
            )?;
        }
        Ok(())
    }
}
pub(crate) mod literals {
    use super::local_storage::{address, store};
    use super::{
        Builder, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
        SelectedInstructionKind, VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use semantic_vocabulary::{IntegerValue, StructuralPlaceKind};

    pub(crate) fn establish(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
            destination,
            structural_type,
            bytes,
        } = &row.kind
        else {
            return Err(invalid());
        };
        if row.result.is_some()
            || !matches!(destination.kind, StructuralPlaceKind::ByteSequenceLiteral { structural_type: identity, .. } if identity == structural_type.id)
            || !structural_type.shape.is_borrowed_byte_view()
            || builder
                .transport
                .pointers
                .iter()
                .any(|(place, _)| *place == destination.id)
        {
            return Err(invalid());
        }
        let length = u64::try_from(bytes.len()).map_err(|_| invalid())?;
        let slot =
            builder
                .transport
                .push_structural_local_slot(row.operation, destination.id, 16, 8);
        let backing = transport_register(builder, destination.id, 16)?;
        builder.emit(
            SelectedInstructionKind::ImageAddress {
                reference: selected_instructions::SelectedImageAddressReference::ByteLiteral {
                    operation: row.operation,
                    place: destination.id,
                },
            },
            builder
                .constraints
                .keys
                .byte_literal_image_address
                .ok_or_else(|| invalid())?,
            &[backing],
            provenance(row),
        )?;
        store(builder, row, slot, 0, backing)?;
        let length = constant(builder, row, 8, length)?;
        store(builder, row, slot, 8, length)?;
        let descriptor = address(builder, row, slot, 0, 16, true)?;
        builder
            .transport
            .pointers
            .push((destination.id, descriptor));
        Ok(())
    }

    fn constant(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        offset: u32,
        value: u64,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { destination, .. } =
            &row.kind
        else {
            return Err(invalid());
        };
        let register = transport_register(builder, destination.id, offset)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(value)),
            register,
            provenance(row),
        )?;
        Ok(register)
    }
}
pub(crate) mod local_storage {
    use super::{Builder, PlaceId, SelectedMemoryAccessRole, VirtualRegisterId, memory};
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};

    fn storage_origin_place(
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        slot: LocalStorageSlotId,
    ) -> Option<PlaceId> {
        let argument_index = match (slot, row.invocation_site()) {
            (
                LocalStorageSlotId::StructuralCallArgument {
                    operation,
                    argument_index,
                },
                semantic_vocabulary::InvocationSite::Operation(site),
            ) if operation == site => argument_index,
            (
                LocalStorageSlotId::InvocationArgument {
                    edge,
                    argument_index,
                },
                semantic_vocabulary::InvocationSite::InvokeNoNormal(site),
            ) if edge == site => argument_index,
            (
                LocalStorageSlotId::StructuralCallArgument { .. }
                | LocalStorageSlotId::InvocationArgument { .. },
                _,
            ) => return None,
            _ => return slot.structural_place(),
        };
        row.invocation_argument_place(usize::try_from(argument_index).ok()?)
    }

    pub(crate) struct PresentedDescriptor {
        pub address: VirtualRegisterId,
        pub words: Option<[VirtualRegisterId; 2]>,
    }

    pub(crate) fn descriptor(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        slot: LocalStorageSlotId,
        first_word: VirtualRegisterId,
        second_word: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        builder
            .transport
            .local_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
        store(builder, row, slot, 0, first_word)?;
        store(builder, row, slot, 8, second_word)?;
        address(builder, row, slot, 0, 16, false)
    }

    pub(crate) fn fixed_array_argument(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: PlaceId,
        argument_index: u32,
        backing: VirtualRegisterId,
        length: u64,
        descriptor_byte_size: u32,
    ) -> Result<PresentedDescriptor, SelectedInstructionError> {
        if !matches!(descriptor_byte_size, 8 | 16) {
            return Err(SelectedInstructionError::custody());
        }
        let slot = crate::selection::construction::scalar_graph::call_arguments::argument_slot(
            row,
            argument_index,
        );
        if descriptor_byte_size == 16 {
            let count = transport_register(builder, place, 8)?;
            builder.emit_materialize_i64(
                semantic_vocabulary::IntegerValue::Unsigned(u128::from(length)),
                count,
                provenance(row),
            )?;
            return Ok(PresentedDescriptor {
                address: descriptor(builder, row, slot, backing, count)?,
                words: Some([backing, count]),
            });
        }
        builder
            .transport
            .local_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: descriptor_byte_size,
                alignment: 8,
            });
        store(builder, row, slot, 0, backing)?;
        Ok(PresentedDescriptor {
            address: address(builder, row, slot, 0, descriptor_byte_size, false)?,
            words: None,
        })
    }

    pub(crate) fn byte_field_argument(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: PlaceId,
        argument_index: u32,
        field: VirtualRegisterId,
        field_offset: u32,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Result<PresentedDescriptor, SelectedInstructionError> {
        let slot = crate::selection::construction::scalar_graph::call_arguments::argument_slot(
            row,
            argument_index,
        );
        let length = transport_register(builder, place, field_offset)?;
        super::runtime_address::footprint(
            builder,
            row,
            place,
            field_offset,
            8,
            indices,
            super::runtime_address::Written::None,
        )?;
        builder.emit_load64(0, field, length, provenance(row))?;
        let data = transport_register(
            builder,
            place,
            field_offset.checked_add(8).ok_or_else(|| invalid())?,
        )?;
        builder.emit_address_offset(8, field, data, provenance(row))?;
        Ok(PresentedDescriptor {
            address: descriptor(builder, row, slot, data, length)?,
            words: Some([data, length]),
        })
    }

    pub(crate) fn store(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        slot: LocalStorageSlotId,
        offset: u32,
        value: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        memory(
            builder,
            row,
            storage_origin_place(row, slot).ok_or_else(|| invalid())?,
            offset,
            8,
            SelectedMemoryAccessRole::WriteLocal { slot },
        )?;
        builder.emit_store64(
            FrameStorageSlotId::Local(slot),
            offset,
            value,
            provenance(row),
        )
    }

    pub(crate) fn address(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        slot: LocalStorageSlotId,
        offset: u32,
        byte_count: u32,
        settles_work: bool,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let register = transport_register(
            builder,
            storage_origin_place(row, slot).ok_or_else(|| invalid())?,
            offset,
        )?;
        memory(
            builder,
            row,
            storage_origin_place(row, slot).ok_or_else(|| invalid())?,
            offset,
            byte_count,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        let mut provenance = provenance(row);
        if settles_work {
            provenance.work = row.invocation_work().to_vec();
        }
        builder.emit_frame_address(FrameStorageSlotId::Local(slot), register, provenance)?;
        Ok(register)
    }
}
pub(crate) use crate::selection_interrupts::static_interrupts;

pub(crate) fn establish_atomic_outcome(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    observed: VirtualRegisterId,
    expected: VirtualRegisterId,
    scalar: ScalarType,
) -> Result<(), SelectedInstructionError> {
    atomic_outcome::establish(source, row, builder, observed, expected, scalar)
}
pub(crate) mod activation_storage {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, SelectedInstructionKind, SelectedMemoryAccessRole, memory,
        provenance, transport_register,
    };
    use crate::SelectedInstructionError;
    use semantic_vocabulary::IntegerValue;

    pub(crate) fn emit(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let invalid = SelectedInstructionError::custody;
        if matches!(
            row.kind,
            LegalizedScalarInstructionKind::SplitActivationClaim { .. }
                | LegalizedScalarInstructionKind::MergeActivationClaim { .. }
        ) {
            return emit_division(builder, row);
        }
        if let LegalizedScalarInstructionKind::ReleaseActivationClaim { source, .. } = &row.kind {
            let anchor = transport_register(builder, source.place, 0)?;
            let mut origin = provenance(row);
            origin.work = row.work.clone();
            return builder.emit_materialize_i64(IntegerValue::Unsigned(0), anchor, origin);
        }
        let LegalizedScalarInstructionKind::EstablishActivationClaim {
            result,
            bound,
            committed,
            established_case,
            rejected_case,
            layout,
            ..
        } = &row.kind
        else {
            return Err(invalid());
        };
        let declarations = &source
            .structural
            .as_ref()
            .ok_or_else(invalid)?
            .structural_types;
        let declaration = declarations
            .iter()
            .find(|ty| ty.id == result.structural_type)
            .ok_or_else(invalid)?;
        let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
            return Err(invalid());
        };
        if terminal_psi::structural_case_position(cases, *established_case) != Some(0)
            || terminal_psi::structural_case_position(cases, *rejected_case) != Some(1)
            || cases.len() != 2
            || cases[0].fields.len() != 1
            || !cases[1].fields.is_empty()
            || layout.cases.len() != 2
            || layout.cases[0].fields.len() != 1
        {
            return Err(invalid());
        }
        let bound_bytes = u32::try_from(*bound).map_err(|_| invalid())?;
        let backing_offset = layout
            .shape
            .byte_size
            .checked_add(7)
            .map(|size| size & !7)
            .ok_or_else(invalid)?;
        let payload_offset = backing_offset.checked_add(8).ok_or_else(invalid)?;
        let storage_shape =
            crate::selection::activation_storage_input::footprint(layout.shape, *bound)
                .ok_or_else(invalid)?;
        let total = payload_offset
            .checked_add(bound_bytes)
            .ok_or_else(invalid)?;
        if total != storage_shape.byte_size {
            return Err(invalid());
        }
        let slot = builder.transport.push_structural_local_slot(
            row.operation,
            result.place,
            total,
            storage_shape.alignment,
        );
        let pointer = super::local_storage::address(builder, row, slot, 0, total, true)?;
        let count = transport_register(builder, result.place, 0)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(total)),
            count,
            provenance(row),
        )?;
        let cursor = super::byte_field_store::scratch(builder, 2)?;
        memory(
            builder,
            row,
            result.place,
            0,
            total,
            SelectedMemoryAccessRole::WriteLocal { slot },
        )?;
        builder.emit(
            SelectedInstructionKind::ZeroBytes,
            builder.constraints.keys.zero_bytes.ok_or_else(invalid)?,
            &[pointer, count, cursor],
            provenance(row),
        )?;
        let (_, committed_register, _, committed_type) =
            builder.resolve(*committed).ok_or_else(invalid)?;
        if !matches!(committed_type, super::ScalarType::Integer(integer) if integer.bits() == 64 && integer.sign() == super::IntegerSign::Unsigned && integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        {
            return Err(invalid());
        }
        let bound_register = transport_register(builder, result.place, 0)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(*bound)),
            bound_register,
            provenance(row),
        )?;
        let tag = transport_register(builder, result.place, 0)?;
        builder.emit(
            SelectedInstructionKind::CompareI64,
            builder.constraints.keys.compare_i64,
            &[bound_register, committed_register],
            provenance(row),
        )?;
        builder.emit(
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
            builder.constraints.keys.materialize_boolean,
            &[tag],
            provenance(row),
        )?;
        memory(
            builder,
            row,
            result.place,
            layout.tag_byte_offset,
            4,
            SelectedMemoryAccessRole::WriteLocal { slot },
        )?;
        builder.emit_store(layout.tag_byte_offset, 4, pointer, tag, provenance(row))?;
        let data = transport_register(builder, result.place, payload_offset)?;
        builder.emit_address_offset(payload_offset, pointer, data, provenance(row))?;
        let descriptor_offset = layout.cases[0].fields[0].byte_offset;
        super::local_storage::store(builder, row, slot, descriptor_offset, data)?;
        super::local_storage::store(
            builder,
            row,
            slot,
            descriptor_offset.checked_add(8).ok_or_else(invalid)?,
            committed_register,
        )?;
        super::local_storage::store(builder, row, slot, backing_offset, committed_register)?;
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }

    fn emit_division(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let invalid = SelectedInstructionError::custody;
        let (result, source, layout) = match &row.kind {
            LegalizedScalarInstructionKind::SplitActivationClaim {
                result,
                source,
                layout,
                ..
            } => (result, source, layout),
            LegalizedScalarInstructionKind::MergeActivationClaim {
                result,
                lower,
                layout,
                ..
            } => (result, lower, layout),
            _ => return Err(invalid()),
        };
        if layout.cases.len() != 2 || layout.cases[0].fields.is_empty() {
            return Err(invalid());
        }
        let total = layout.shape.byte_size;
        let slot = builder.transport.push_structural_local_slot(
            row.operation,
            result.place,
            total,
            layout.shape.alignment,
        );
        let pointer = super::local_storage::address(builder, row, slot, 0, total, true)?;
        let count = transport_register(builder, result.place, 0)?;
        let cursor = transport_register(builder, result.place, 1)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(total)),
            count,
            provenance(row),
        )?;
        memory(
            builder,
            row,
            result.place,
            0,
            total,
            SelectedMemoryAccessRole::WriteLocal { slot },
        )?;
        let zero_bytes = builder.constraints.keys.zero_bytes.ok_or_else(invalid)?;
        builder.emit(
            SelectedInstructionKind::ZeroBytes,
            zero_bytes,
            &[pointer, count, cursor],
            provenance(row),
        )?;
        let tag = transport_register(builder, result.place, 0)?;
        builder.emit_materialize_i64(IntegerValue::Unsigned(0), tag, provenance(row))?;
        memory(
            builder,
            row,
            result.place,
            layout.tag_byte_offset,
            4,
            SelectedMemoryAccessRole::WriteLocal { slot },
        )?;
        builder.emit_store(layout.tag_byte_offset, 4, pointer, tag, provenance(row))?;
        let claim = super::place_pointer(builder, row, source.place, 0)?;
        let parent_pointer = transport_register(builder, result.place, 2)?;
        let parent_length = transport_register(builder, result.place, 3)?;
        builder.emit_load64(0, claim, parent_pointer, provenance(row))?;
        builder.emit_load64(8, claim, parent_length, provenance(row))?;
        let fields = if matches!(
            row.kind,
            LegalizedScalarInstructionKind::SplitActivationClaim { .. }
        ) {
            &layout.cases[0].fields
        } else {
            &layout.cases[0].fields[..1]
        };
        for field in fields {
            super::local_storage::store(builder, row, slot, field.byte_offset, parent_pointer)?;
            super::local_storage::store(
                builder,
                row,
                slot,
                field.byte_offset.checked_add(8).ok_or_else(invalid)?,
                parent_length,
            )?;
        }
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }
}
pub(crate) mod native_code_layout {
    use super::local_storage::{address, store};
    use super::{
        Builder, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
        SelectedInstructionKind,
    };
    use super::{invalid, provenance, transport_register};
    use crate::SelectedInstructionError;

    pub(crate) fn establish(
        source: &legalized_operations::LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::EstablishNativeCodeLayout {
            result, machine, ..
        } = &row.kind
        else {
            return Err(invalid());
        };
        if !crate::selection::native_code_layout_input::valid(source, row) || row.result.is_some() {
            return Err(invalid());
        }
        let slot = builder
            .transport
            .push_structural_local_slot(row.operation, result.place, 24, 8);
        let image = transport_register(builder, result.place, 24)?;
        builder.emit(
            SelectedInstructionKind::ImageAddress {
                reference: selected_instructions::SelectedImageAddressReference::NativeCodeLayout {
                    machine: *machine,
                },
            },
            builder
                .constraints
                .keys
                .byte_literal_image_address
                .ok_or_else(invalid)?,
            &[image],
            provenance(row),
        )?;
        for offset in [0, 8, 16] {
            let word = transport_register(builder, result.place, offset)?;
            builder.emit(
                SelectedInstructionKind::Load64 {
                    byte_offset: offset,
                },
                builder.constraints.keys.load64.ok_or_else(invalid)?,
                &[image, word],
                provenance(row),
            )?;
            store(builder, row, slot, offset, word)?;
        }
        let descriptor = address(builder, row, slot, 0, 24, true)?;
        builder.transport.pointers.push((result.place, descriptor));
        Ok(())
    }
}
pub(crate) use local_storage::{byte_field_argument, fixed_array_argument};
pub(crate) mod case_membership {
    use super::runtime_address::{cross_references, projection_origin, scale};
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind, VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::IntegerValue;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use crate::selection::construction::scalar_graph::structural::transport_register;

    pub(crate) fn observe(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::StructuralCaseMembership {
            source: place,
            path,
            case_tag,
            tag_byte_offset,
            indices,
            pointer_slot_list,
            ..
        } = &row.kind
        else {
            return Err(invalid());
        };
        let result = row.result.ok_or_else(|| invalid())?;
        if result.scalar_type != ScalarType::Boolean {
            return Err(invalid());
        }
        let pointer_slots = stored_pointer_slots(source, *pointer_slot_list)?;
        let place = *place;
        let tag = transport_register(builder, place, *tag_byte_offset)?;
        let provenance = operation_provenance(row, vec![result.value]);
        let (pointer, pointer_slots) = projection_origin(
            source,
            builder,
            row,
            place,
            *tag_byte_offset,
            &pointer_slots,
        )?;
        if let Some(pointer) = pointer {
            let (pointer, leaf_index) =
                cross_references(builder, row, place, pointer, indices, pointer_slots)?;
            let indices = indices.get(leaf_index..).ok_or(invalid())?;
            let pointer = scale(builder, row, place, *tag_byte_offset, pointer, indices)?;
            let (pointer, displacement) =
                super::memory_pointer(builder, row, place, pointer, *tag_byte_offset, 4)?;
            super::runtime_address::footprint(
                builder,
                row,
                place,
                *tag_byte_offset,
                4,
                indices,
                super::runtime_address::Written::None,
            )?;
            builder.emit(
                SelectedInstructionKind::Load32 {
                    byte_offset: displacement,
                },
                builder.constraints.keys.load32.ok_or_else(|| invalid())?,
                &[pointer, tag],
                provenance,
            )?;
        } else {
            if !path.is_empty() || *tag_byte_offset != 0 || !indices.is_empty() {
                return Err(invalid());
            }
            let fragment = builder
                .transport
                .fragments
                .iter()
                .find(|(stored, offset, _)| *stored == place && *offset == 0)
                .map(|(_, _, register)| *register)
                .ok_or_else(|| invalid())?;
            builder.emit(
                SelectedInstructionKind::ZeroExtendU32,
                builder.constraints.keys.copy_i64,
                &[fragment, tag],
                provenance,
            )?;
        }
        let expected = transport_register(builder, place, 0)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(*case_tag)),
            expected,
            Default::default(),
        )?;
        builder.emit(
            SelectedInstructionKind::CompareI64,
            builder.constraints.keys.compare_i64,
            &[tag, expected],
            Default::default(),
        )?;
        let output = builder.register(result.value, result.definition_site, ScalarType::Boolean)?;
        builder.emit(
            SelectedInstructionKind::MaterializeBooleanEqual,
            builder.constraints.keys.materialize_boolean,
            &[output],
            crate::selection::construction::scalar_graph::value_provenance(result.value),
        )?;
        Ok(output)
    }

    fn stored_pointer_slots(
        function: &LegalizedScalarFunction,
        list: u32,
    ) -> Result<
        Vec<crate::structural_inputs::structural_reference_input::PointerSlot>,
        SelectedInstructionError,
    > {
        let slots = function
            .case_pointer_slots
            .get(usize::try_from(list).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        slots
            .iter()
            .map(|slot| {
                Ok(
                    crate::structural_inputs::structural_reference_input::PointerSlot {
                        byte_offset: slot.byte_offset,
                        indices: usize::try_from(slot.index_start).map_err(|_| invalid())?
                            ..usize::try_from(slot.index_end).map_err(|_| invalid())?,
                        scalar_descriptor: slot.scalar_descriptor,
                    },
                )
            })
            .collect()
    }
}
pub(crate) mod descriptor_tables {
    use super::primitive_locals::borrowed_storage_address;
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, SelectedInstructionKind, SelectedMemoryAccessRole, invalid,
        memory, provenance, transport_register,
    };
    use crate::SelectedInstructionError;

    pub(crate) fn emit(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<bool, SelectedInstructionError> {
        let (operation, key, mut operands) = match &row.kind {
            LegalizedScalarInstructionKind::ActivateTaskRegister {
                entries,
                gdt,
                task_state,
                selector,
                scratch,
                indices,
                ..
            } => {
                let key = builder
                    .constraints
                    .keys
                    .task_register_activate
                    .ok_or_else(|| {
                        SelectedInstructionError::custody_because(
                            "task-register activation requires an x86-64 target",
                        )
                    })?;
                let (table, table_offset) =
                    borrowed_storage_address(source, builder, row, gdt, &indices[0])?;
                let (task, task_offset) =
                    borrowed_storage_address(source, builder, row, task_state, &indices[1])?;
                let (snapshot, snapshot_offset) =
                    borrowed_storage_address(source, builder, row, scratch, &indices[2])?;
                let (_, selector, _, _) = builder.resolve(*selector).ok_or_else(invalid)?;
                let extent = transport_register(builder, gdt.place, table_offset)?;
                let byte_count = u32::from(*entries) * 8;
                builder.emit_materialize_i64(
                    semantic_vocabulary::IntegerValue::Unsigned(u128::from(byte_count)),
                    extent,
                    provenance(row),
                )?;
                memory(
                    builder,
                    row,
                    gdt.place,
                    table_offset,
                    byte_count,
                    SelectedMemoryAccessRole::ReadPlace,
                )?;
                memory(
                    builder,
                    row,
                    gdt.place,
                    table_offset,
                    byte_count,
                    SelectedMemoryAccessRole::WritePlace,
                )?;
                memory(
                    builder,
                    row,
                    task_state.place,
                    task_offset,
                    104,
                    SelectedMemoryAccessRole::ReadPlace,
                )?;
                memory(
                    builder,
                    row,
                    scratch.place,
                    snapshot_offset,
                    10,
                    SelectedMemoryAccessRole::WritePlace,
                )?;
                (
                    terminal_psi::RegisterOperation::TaskRegisterActivate,
                    key,
                    vec![table, task, selector, snapshot, extent],
                )
            }
            LegalizedScalarInstructionKind::ReloadSegmentDescriptors {
                entries,
                kind,
                gdt,
                selector,
                scratch,
                indices,
                ..
            } => {
                let key = match kind {
                    terminal_psi::SegmentReload::Code => {
                        builder.constraints.keys.code_segment_reload
                    }
                    terminal_psi::SegmentReload::Data => {
                        builder.constraints.keys.data_segment_reload
                    }
                }
                .ok_or_else(|| {
                    SelectedInstructionError::custody_because(
                        "selector reload requires an x86-64 target",
                    )
                })?;
                let (table, table_offset) =
                    borrowed_storage_address(source, builder, row, gdt, &indices[0])?;
                let (snapshot, snapshot_offset) =
                    borrowed_storage_address(source, builder, row, scratch, &indices[1])?;
                let (_, selector, _, _) = builder.resolve(*selector).ok_or_else(invalid)?;
                let extent = transport_register(builder, gdt.place, table_offset)?;
                let byte_count = u32::from(*entries) * 8;
                builder.emit_materialize_i64(
                    semantic_vocabulary::IntegerValue::Unsigned(u128::from(byte_count)),
                    extent,
                    provenance(row),
                )?;
                memory(
                    builder,
                    row,
                    gdt.place,
                    table_offset,
                    byte_count,
                    SelectedMemoryAccessRole::ReadPlace,
                )?;
                memory(
                    builder,
                    row,
                    gdt.place,
                    table_offset,
                    byte_count,
                    SelectedMemoryAccessRole::WritePlace,
                )?;
                memory(
                    builder,
                    row,
                    scratch.place,
                    snapshot_offset,
                    10,
                    SelectedMemoryAccessRole::WritePlace,
                )?;
                (
                    terminal_psi::RegisterOperation::SegmentReload(*kind),
                    key,
                    vec![table, selector, snapshot, extent],
                )
            }
            LegalizedScalarInstructionKind::ReadSegmentSelector { register, .. } => {
                let key = builder
                    .constraints
                    .keys
                    .segment_selector_read
                    .ok_or_else(|| {
                        SelectedInstructionError::custody_because(
                            "selector read requires an x86-64 target",
                        )
                    })?;
                (
                    terminal_psi::RegisterOperation::SegmentRead(*register),
                    key,
                    Vec::new(),
                )
            }
            LegalizedScalarInstructionKind::ReadTaskRegister { .. } => {
                let key = builder.constraints.keys.task_register_read.ok_or_else(|| {
                    SelectedInstructionError::custody_because(
                        "task-register read requires an x86-64 target",
                    )
                })?;
                (
                    terminal_psi::RegisterOperation::TaskRegisterRead,
                    key,
                    Vec::new(),
                )
            }
            _ => return Ok(false),
        };
        let result = row.result.ok_or_else(invalid)?;
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        operands.push(output);
        builder.emit(
            SelectedInstructionKind::RegisterAccess { operation },
            key,
            &operands,
            provenance(row),
        )?;
        builder.definitions.push((
            result.value,
            output,
            result.definition_site,
            result.scalar_type,
        ));
        Ok(true)
    }
}
pub(crate) use crate::selection_views_and_bytes::element_establishment;
pub(crate) use crate::selection_views_and_bytes::element_subslice;
pub(crate) use crate::selection_views_and_bytes::element_view_write;
pub(crate) use crate::selection_views_and_bytes::element_views;
pub(crate) mod primitive_locals {
    use super::runtime_address::{
        cross_references, footprint, projection_origin, scalar_reference, scale,
    };
    use super::{
        Builder, IntegerSign, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, ScalarType, SelectedMemoryAccessRole, VirtualRegisterId,
        memory,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::local_storage;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use crate::selection::primitive_local_input::{
        read_geometry, read_storage_codec, read_storage_fragments,
    };
    use crate::selection::scalar_call_abi::load_transfer;
    use selected_instructions::LocalStorageSlotId;

    pub(crate) fn write(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let (place, value, establishing) = match &row.kind {
            LegalizedScalarInstructionKind::EstablishPrimitiveLocal { result, value, .. } => {
                (result.place, *value, true)
            }
            LegalizedScalarInstructionKind::PrimitiveLocalStore { destination, value } => {
                (*destination, *value, false)
            }
            _ => return Err(invalid()),
        };
        let (producer, _, scalar, shape) =
            crate::selection::primitive_local_input::local(source, place)
                .ok_or_else(|| invalid())?;
        if row.result.is_some() || scalar != value.scalar_type {
            return Err(invalid());
        }
        let pointer = if establishing {
            if producer != row.operation
                || builder
                    .transport
                    .pointers
                    .iter()
                    .any(|(stored, _)| *stored == place)
            {
                return Err(invalid());
            }
            let slot = LocalStorageSlotId::Structural {
                operation: producer,
                place,
            };
            builder.transport.push_structural_local_slot(
                producer,
                place,
                shape.byte_size,
                shape.alignment,
            );
            let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size, false)?;
            builder.transport.pointers.push((place, pointer));
            pointer
        } else {
            builder
                .transport
                .pointers
                .iter()
                .find(|(stored, _)| *stored == place)
                .map(|(_, pointer)| *pointer)
                .ok_or_else(|| invalid())?
        };
        let (_, value_register, _, actual) =
            builder.resolve(value.value).ok_or_else(|| invalid())?;
        if actual != scalar {
            return Err(invalid());
        }
        let byte_size = u8::try_from(shape.byte_size).map_err(|_| invalid())?;
        memory(
            builder,
            row,
            place,
            0,
            u32::from(byte_size),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(
            0,
            byte_size,
            pointer,
            value_register,
            operation_provenance(row, vec![value.value]),
        )?;
        Ok(())
    }
    pub(crate) fn read(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let (place, byte_offset, pointer_slots) =
            read_geometry(source, row).ok_or_else(|| invalid())?;
        let definition = row.result.ok_or_else(|| invalid())?;
        let observes_descriptor_home = matches!(&row.kind,
            LegalizedScalarInstructionKind::ElementViewLength { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
            if source.path.is_empty());
        let (pointer, pointer_slots) = if observes_descriptor_home {
            (
                Some(super::place_pointer(builder, row, place, 0)?),
                pointer_slots.as_slice(),
            )
        } else {
            projection_origin(source, builder, row, place, byte_offset, &pointer_slots)?
        };
        let pointer = pointer.ok_or_else(invalid)?;
        let pointer = if matches!(&row.kind,
            LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, .. }
            if matches!(source.path.first(), Some(terminal_psi::StructuralPathSegment::Referent)))
        {
            super::reference::load(builder, row, place, pointer)?
        } else {
            pointer
        };
        let indices: &[legalized_operations::LegalizedRuntimeIndexOperand] = match &row.kind {
            LegalizedScalarInstructionKind::PrimitiveScalarRead { indices, .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                indices, ..
            }
            | LegalizedScalarInstructionKind::ElementViewLength { indices, .. }
            | LegalizedScalarInstructionKind::ByteSequenceLength { indices, .. } => indices,
            _ => &[],
        };
        if let Some((address, codec)) =
            scalar_reference(source, builder, row, place, pointer, indices, pointer_slots)?
        {
            if byte_offset != 0 {
                return Err(invalid());
            }
            return super::encoded_scalar::read(builder, row, place, address, codec);
        }
        let (pointer, leaf_index) =
            cross_references(builder, row, place, pointer, indices, pointer_slots)?;
        let indices = &indices[leaf_index..];
        let pointer = scale(builder, row, place, byte_offset, pointer, indices)?;
        if let Some(fragments) = read_storage_fragments(source, row) {
            return super::fragmented_scalar::read(
                builder,
                row,
                place,
                pointer,
                byte_offset,
                fragments,
                indices,
            );
        }
        let codec = read_storage_codec(source, row).ok_or_else(|| invalid())?;
        let canonical = runtime_abi::ScalarStorageCodec::from_scalar(definition.scalar_type)
            .ok_or_else(|| invalid())?;
        if codec != canonical || matches!(codec.byte_size(), 3 | 5 | 6 | 7) {
            let address =
                super::encoded_scalar::backing_address(builder, row, place, pointer, byte_offset)?;
            let codec = super::encoded_scalar::constant_codec(builder, row, place, codec)?;
            return super::encoded_scalar::read(builder, row, place, address, codec);
        }
        let shape = crate::selection::scalar_call_abi::scalar_shape(definition.scalar_type)
            .ok_or_else(|| invalid())?;
        let (pointer, displacement) =
            super::memory_pointer(builder, row, place, pointer, byte_offset, shape.byte_size)?;
        let output = builder.register(
            definition.value,
            definition.definition_site,
            definition.scalar_type,
        )?;
        let (instruction, constraint) =
            load_transfer(&builder.constraints.keys, shape.byte_size, displacement)
                .ok_or_else(invalid)?;
        footprint(
            builder,
            row,
            place,
            byte_offset,
            shape.byte_size,
            indices,
            super::runtime_address::Written::None,
        )?;
        builder.emit(
            instruction,
            constraint.ok_or_else(|| invalid())?,
            &[pointer, output],
            operation_provenance(
                row,
                indices
                    .iter()
                    .map(|index| index.operand.value)
                    .chain([definition.value])
                    .collect(),
            ),
        )?;
        normalize_signed_load(builder, definition, output)
    }

    pub(crate) fn normalize_signed_load(
        builder: &mut Builder<'_>,
        definition: legalized_operations::LegalizedValueDefinition,
        output: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if !matches!(definition.scalar_type, ScalarType::Integer(integer)
            if integer.sign() == IntegerSign::Signed && matches!(integer.bits(), 8 | 16 | 32))
        {
            return Ok(output);
        }
        let normalized = builder.register(
            definition.value,
            definition.definition_site,
            definition.scalar_type,
        )?;
        builder.emit(
            crate::selection::scalar_call_abi::integer_carrier_normalization(
                definition.scalar_type,
            ),
            builder.constraints.keys.copy_i64,
            &[output, normalized],
            crate::selection::construction::scalar_graph::value_provenance(definition.value),
        )?;
        Ok(normalized)
    }

    pub(crate) fn address(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let address = projected_address(source, builder, row)?;
        let result = row.result.ok_or_else(|| {
            SelectedInstructionError::custody_because(
                "storage address lost its declared addr result",
            )
        })?;
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        builder.emit_copy_i64(address, output, super::provenance(row))?;
        Ok(output)
    }

    fn projected_address(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let (place, byte_offset, slots) = read_geometry(source, row).ok_or_else(|| {
            SelectedInstructionError::custody_because(
                "storage address lost its checked referent geometry",
            )
        })?;
        let indices = match &row.kind {
            LegalizedScalarInstructionKind::StorageAddress { indices, .. }
            | LegalizedScalarInstructionKind::GlobalDescriptorTable { indices, .. } => indices,
            _ => return Err(invalid()),
        };
        resolve_projected_address(source, builder, row, place, byte_offset, indices, &slots)
    }

    pub(crate) fn borrowed_storage_address(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        argument: &terminal_psi::StructuralArgument,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Result<(VirtualRegisterId, u32), SelectedInstructionError> {
        let signature = source.structural.as_ref().ok_or_else(invalid)?;
        let root = crate::selection::reference_input::root_type(source, argument.place)
            .ok_or_else(invalid)?;
        let geometry =
            crate::structural_inputs::structural_reference_input::storage_address_geometry(
                root,
                &argument.path,
                &signature.structural_types,
            )
            .ok_or_else(invalid)?;
        if !crate::structural_inputs::runtime_indices_match(indices, &geometry.elements) {
            return Err(invalid());
        }
        let address = resolve_projected_address(
            source,
            builder,
            row,
            argument.place,
            geometry.byte_offset,
            indices,
            &geometry.pointer_slots,
        )?;
        Ok((address, geometry.byte_offset))
    }

    fn resolve_projected_address(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: semantic_vocabulary::PlaceId,
        byte_offset: u32,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
        slots: &[crate::structural_inputs::structural_reference_input::PointerSlot],
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let (pointer, slots) = projection_origin(source, builder, row, place, byte_offset, slots)?;
        let pointer = match pointer {
            Some(pointer) => pointer,
            None => super::place_pointer(builder, row, place, 0)?,
        };
        let address = if let Some((address, _)) =
            scalar_reference(source, builder, row, place, pointer, indices, slots)?
        {
            if byte_offset != 0 {
                return Err(invalid());
            }
            address
        } else {
            let (pointer, leaf) = cross_references(builder, row, place, pointer, indices, slots)?;
            let pointer = scale(builder, row, place, byte_offset, pointer, &indices[leaf..])?;
            super::encoded_scalar::backing_address(builder, row, place, pointer, byte_offset)?
        };
        Ok(address)
    }

    pub(crate) fn global_descriptor_table(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<bool, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::GlobalDescriptorTable {
            operation,
            source: place,
            ..
        } = row.kind
        else {
            return Ok(false);
        };
        if row.result.is_some() {
            return Err(invalid());
        }
        let key = builder
            .constraints
            .keys
            .global_descriptor_table
            .ok_or_else(invalid)?;
        let address = projected_address(source, builder, row)?;
        let (_, offset, _) = read_geometry(source, row).ok_or_else(invalid)?;
        let role = match operation {
            terminal_psi::GlobalDescriptorTableOperation::Store => {
                SelectedMemoryAccessRole::WritePlace
            }
            terminal_psi::GlobalDescriptorTableOperation::Load => {
                SelectedMemoryAccessRole::ReadPlace
            }
        };
        memory(builder, row, place, offset, 10, role)?;
        builder.emit(
            selected_instructions::SelectedInstructionKind::RegisterAccess {
                operation: terminal_psi::RegisterOperation::GlobalDescriptorTable(operation),
            },
            key,
            &[address],
            super::provenance(row),
        )?;
        Ok(true)
    }
}
pub(crate) use case_membership::observe;
pub(crate) use descriptor_tables::emit as descriptor_tables;
pub(crate) mod indirect_results {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction, PlaceId,
        SelectedInstructionKind, SelectedInstructionProvenance, VirtualRegisterId,
    };
    use crate::SelectedInstructionError;
    use crate::selection::aggregate_result_input::{call_completion, indirect_result};
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::local_storage;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};

    pub(crate) fn entry(
        source: &LegalizedScalarFunction,
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let Some(placement) = source.call_plan.result.as_ref() else {
            return Ok(());
        };
        let Some(register) = indirect_result(placement, source.call_plan.policy) else {
            return Ok(());
        };
        if source.call_plan.policy
            != calling_conventions::CallingPolicy::native_for_target(environment.target())
        {
            return Err(invalid());
        }
        let signature = source.structural.as_ref().ok_or_else(|| invalid())?;
        let result = signature.result.as_ref().ok_or_else(|| invalid())?;
        if crate::structural_inputs::shape(result.structural_type, &signature.structural_types)
            != Some(placement.shape)
        {
            return Err(invalid());
        }
        let fixed = environment
            .fixed_register_view(register)
            .ok_or_else(|| invalid())?;
        let input = transport_register(builder, result.place, 0)?;
        builder.registers[input.0 as usize].entry_fixed_view = Some(fixed);
        let retained = transport_register(builder, result.place, 0)?;
        builder.emit_copy_i64(input, retained, Default::default())?;
        builder.transport.result_pointer = Some(retained);
        Ok(())
    }

    pub(crate) fn prepare_call(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
        let Some((result, placement, policy)) = call_completion(source, row) else {
            return Ok(None);
        };
        if indirect_result(placement, policy).is_none() {
            return Ok(None);
        }
        let slot = LocalStorageSlotId::Structural {
            operation: row.operation,
            place: result.place,
        };
        if builder
            .transport
            .local_slots
            .iter()
            .any(|existing| existing.id == slot)
        {
            return Err(invalid());
        }
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: placement.shape.byte_size,
                alignment: placement.shape.alignment,
            });
        let pointer =
            local_storage::address(builder, row, slot, 0, placement.shape.byte_size, false)?;
        Ok(Some(pointer))
    }

    pub(crate) fn finish_call(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let (result, placement, _) = call_completion(source, row).ok_or_else(|| invalid())?;
        let slot = LocalStorageSlotId::Structural {
            operation: row.operation,
            place: result.place,
        };
        let pointer =
            local_storage::address(builder, row, slot, 0, placement.shape.byte_size, false)?;
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }

    pub(crate) fn returned(
        source: &LegalizedScalarFunction,
        block: &legalized_operations::LegalizedScalarBlock,
        returned: &legalized_operations::LegalizedScalarReturn,
        place: PlaceId,
        input: VirtualRegisterId,
        placement: &calling_conventions::ValuePlacement,
        builder: &mut Builder<'_>,
        cleanup_calls: &[(
            target_operations::CallSiteOwner,
            legalized_operations::LegalizedScalarCall,
        )],
        environment: &register_environment::ValidatedTargetRegisterEnvironment,
    ) -> Result<(), SelectedInstructionError> {
        let destination = source
            .structural
            .as_ref()
            .and_then(|signature| signature.result.as_ref())
            .ok_or_else(|| invalid())?
            .place;
        let pointer = builder.transport.result_pointer.ok_or_else(|| invalid())?;
        super::byte_copy::copy(
            builder,
            selected_instructions::SelectedMemoryAccessOrigin::Block(block.id),
            Default::default(),
            super::byte_copy::CopyEnd {
                place,
                pointer: input,
                byte_offset: 0,
                elements: &[],
            },
            super::byte_copy::CopyEnd {
                place: destination,
                pointer,
                byte_offset: 0,
                elements: &[],
            },
            placement.shape.byte_size,
        )?;
        for (owner, call) in cleanup_calls {
            crate::selection::construction::scalar_graph::cleanup_call::emit(
                *owner,
                call,
                returned.effect,
                builder,
                environment,
            )?;
        }
        let returned_pointer = matches!(
            source.call_plan.policy,
            calling_conventions::CallingPolicy::MicrosoftX64
                | calling_conventions::CallingPolicy::SystemVAMD64
        );
        let pointer = if returned_pointer {
            let output = transport_register(builder, destination, 0)?;
            builder.emit_copy_i64(pointer, output, Default::default())?;
            output
        } else {
            pointer
        };
        builder.emit(
            if returned_pointer {
                SelectedInstructionKind::ReturnScalar
            } else {
                SelectedInstructionKind::ReturnUnit
            },
            if returned_pointer {
                builder.constraints.keys.return_i64
            } else {
                builder.constraints.keys.return_unit
            },
            if returned_pointer {
                std::slice::from_ref(&pointer)
            } else {
                &[]
            },
            SelectedInstructionProvenance {
                edges: vec![returned.edge],
                work: returned.work.clone(),
                ..Default::default()
            },
        )?;
        Ok(())
    }
}
pub(crate) mod record {
    use super::encoded_scalar::{backing_address, constant_codec};
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, PlaceId, SelectedInstructionProvenance,
        SelectedMemoryAccessRole, VirtualRegisterId, memory,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::local_storage;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::selection::construction::scalar_graph::{IntegerValue, ValueId};
    use crate::structural_inputs::structural_reference_input::{
        scalar_field_codec, scalar_field_fragments,
    };
    pub(crate) fn establish(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let fields =
            crate::selection::record_input::fields(source, row).ok_or_else(|| invalid())?;
        let LegalizedScalarInstructionKind::EstablishRecord {
            result,
            fields: initializers,
            shape,
        } = &row.kind
        else {
            return Err(invalid());
        };
        let pointer = initialize_storage(row, builder, result.place, *shape)?;
        if shape.byte_size == 0 {
            builder.transport.pointers.push((result.place, pointer));
            return Ok(());
        }
        write_fields(
            source,
            row,
            builder,
            result.place,
            result.structural_type,
            pointer,
            initializers,
            &fields,
        )?;
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }
    pub(crate) fn initialize_storage(
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
        destination: PlaceId,
        shape: calling_conventions::ValueShape,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let slot = builder.transport.push_structural_local_slot(
            row.operation,
            destination,
            shape.byte_size.max(1),
            shape.alignment,
        );
        let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size.max(1), true)?;
        if shape.byte_size == 0 {
            return Ok(pointer);
        }
        if shape.byte_size > super::zero_fill::INLINE_HOME_MAX_BYTES {
            super::zero_fill::whole_home(builder, row, destination, pointer, shape.byte_size)?;
            return Ok(pointer);
        }
        let zero = transport_register(builder, destination, 0)?;
        builder.emit_materialize_i64(IntegerValue::Unsigned(0), zero, provenance(row))?;
        let mut offset = 0u32;
        while offset < shape.byte_size {
            let width = chunk(shape.byte_size - offset, offset, offset);
            store(
                builder,
                row,
                destination,
                pointer,
                offset,
                width,
                zero,
                Vec::new(),
                Vec::new(),
            )?;
            offset += u32::from(width);
        }
        Ok(pointer)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_fields(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
        destination: PlaceId,
        parent: semantic_vocabulary::StructuralTypeId,
        pointer: VirtualRegisterId,
        initializers: &[terminal_psi::RecordFieldInitializer],
        fields: &[(
            u32,
            calling_conventions::ValueShape,
            Option<semantic_vocabulary::ScalarType>,
        )],
    ) -> Result<(), SelectedInstructionError> {
        if initializers.len() != fields.len() {
            return Err(invalid());
        }
        for (initializer, (offset, field_shape, expected_scalar)) in
            initializers.iter().zip(fields.iter().copied())
        {
            match &initializer.value {
                terminal_psi::RecordFieldValue::Scalar {
                    value,
                    range_obligation,
                } => {
                    let (_, register, _, scalar) =
                        builder.resolve(*value).ok_or_else(|| invalid())?;
                    if Some(scalar) != expected_scalar {
                        return Err(invalid());
                    }
                    let declarations = &source
                        .structural
                        .as_ref()
                        .ok_or_else(invalid)?
                        .structural_types;
                    if let Some(fragments) =
                        scalar_field_fragments(parent, &[], initializer.field, declarations)
                    {
                        super::fragmented_scalar::write(
                            builder,
                            row,
                            destination,
                            pointer,
                            offset,
                            fragments,
                            &[],
                            *value,
                        )?;
                        continue;
                    }
                    let codec = scalar_field_codec(parent, &[], initializer.field, declarations);
                    if let Some(codec) = codec {
                        let canonical = runtime_abi::ScalarStorageCodec::from_scalar(scalar)
                            .ok_or_else(invalid)?;
                        if u32::from(codec.byte_size()) != field_shape.byte_size {
                            return Err(invalid());
                        }
                        if codec != canonical || matches!(codec.byte_size(), 3 | 5 | 6 | 7) {
                            let address =
                                backing_address(builder, row, destination, pointer, offset)?;
                            let codec = constant_codec(builder, row, destination, codec)?;
                            super::encoded_scalar::write(
                                builder,
                                row,
                                destination,
                                address,
                                codec,
                                abstract_operations::AbstractResult {
                                    value: *value,
                                    scalar_type: scalar,
                                },
                            )?;
                            continue;
                        }
                    }
                    if codec.is_none()
                        && crate::selection::scalar_call_abi::scalar_shape(scalar)
                            != Some(field_shape)
                    {
                        return Err(invalid());
                    }
                    let width = u8::try_from(field_shape.byte_size).map_err(|_| invalid())?;
                    store(
                        builder,
                        row,
                        destination,
                        pointer,
                        offset,
                        width,
                        register,
                        vec![*value],
                        range_obligation.iter().copied().collect(),
                    )?;
                }
                terminal_psi::RecordFieldValue::Structural(argument) => {
                    write_structural_operand(
                        source,
                        row,
                        builder,
                        destination,
                        pointer,
                        argument,
                        offset,
                        field_shape,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn chunk(remaining: u32, source_offset: u32, destination_offset: u32) -> u8 {
        [8u8, 4, 2, 1]
            .into_iter()
            .find(|width| {
                let width = u32::from(*width);
                width <= remaining
                    && source_offset.is_multiple_of(width)
                    && destination_offset.is_multiple_of(width)
            })
            .unwrap_or(1)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn store(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        offset: u32,
        width: u8,
        value: VirtualRegisterId,
        values: Vec<ValueId>,
        obligations: Vec<semantic_vocabulary::ObligationId>,
    ) -> Result<(), SelectedInstructionError> {
        let (pointer, displacement) =
            super::memory_pointer(builder, row, place, pointer, offset, u32::from(width))?;
        memory(
            builder,
            row,
            place,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(
            displacement,
            width,
            pointer,
            value,
            SelectedInstructionProvenance {
                values,
                obligations,
                ..provenance(row)
            },
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_structural_operand(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
        destination: PlaceId,
        pointer: VirtualRegisterId,
        argument: &terminal_psi::StructuralArgument,
        offset: u32,
        field_shape: calling_conventions::ValueShape,
    ) -> Result<(), SelectedInstructionError> {
        super::leaf_copy::copy_operand(
            source,
            row,
            builder,
            argument,
            &[],
            super::leaf_copy::CopyEnd {
                place: destination,
                pointer,
                byte_offset: offset,
                elements: &[],
            },
            field_shape,
        )
    }
}
pub(crate) mod reference {
    use super::runtime_address::{cross_references, footprint, projection_origin, scale};
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, SelectedMemoryAccessRole, invalid, memory, place_pointer,
        provenance, transport_at_byte_offset, transport_register,
    };
    use crate::SelectedInstructionError;

    pub(crate) fn establish(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let address =
            crate::selection::reference_input::establishment(source, row).ok_or(invalid())?;
        let result = match &row.kind {
            LegalizedScalarInstructionKind::EstablishReference { result, .. }
            | LegalizedScalarInstructionKind::EstablishAccessReference { result, .. } => result,
            _ => return Err(invalid()),
        };
        if builder
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == result.place)
            || builder
                .transport
                .local_slots
                .iter()
                .any(|slot| slot.id.structural_place() == Some(result.place))
        {
            return Err(invalid());
        }
        let (input, slots) = if address.pointer_slots.is_empty() {
            (
                place_pointer(builder, row, address.place, 0)?,
                address.pointer_slots.as_slice(),
            )
        } else {
            let (input, slots) = projection_origin(
                source,
                builder,
                row,
                address.place,
                address.byte_offset,
                &address.pointer_slots,
            )?;
            (input.ok_or_else(invalid)?, slots)
        };
        let (mut input, leaf_index) =
            cross_references(builder, row, address.place, input, &address.indices, slots)?;
        input = scale(
            builder,
            row,
            address.place,
            address.byte_offset,
            input,
            &address.indices[leaf_index..],
        )?;
        if address.byte_offset != 0 {
            let projected = transport_register(builder, result.place, address.byte_offset)?;
            builder.emit_address_offset(address.byte_offset, input, projected, provenance(row))?;
            input = projected;
        }
        if let Some(storage) = address.storage {
            let (_, offset, _, scalar) = builder.resolve(storage.byte_offset).ok_or(invalid())?;
            if !matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64)
            {
                return Err(invalid());
            }
            let projected = transport_register(builder, result.place, 0)?;
            builder.emit_byte_view_address(input, offset, projected, provenance(row))?;
            input = projected;
        }
        let value = if address.load_pointer {
            let value = transport_register(builder, result.place, 0)?;
            memory(
                builder,
                row,
                address.place,
                address.byte_offset,
                8,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            builder.emit_load64(0, input, value, provenance(row))?;
            value
        } else {
            input
        };
        let codec = if address.load_codec {
            let codec = transport_register(builder, result.place, 8)?;
            memory(
                builder,
                row,
                address.place,
                address.byte_offset.checked_add(8).ok_or(invalid())?,
                8,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            builder.emit_load64(8, input, codec, provenance(row))?;
            Some(codec)
        } else if let Some(codec) = address.codec {
            let register = transport_register(builder, result.place, 8)?;
            builder.emit_materialize_i64(
                semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                    codec.encode().ok_or(invalid())?,
                )),
                register,
                provenance(row),
            )?;
            Some(register)
        } else {
            None
        };
        let byte_size = if codec.is_some() { 16 } else { 8 };
        let slot =
            builder
                .transport
                .push_structural_local_slot(row.operation, result.place, byte_size, 8);
        let home = super::local_storage::address(builder, row, slot, 0, byte_size, true)?;
        if let Some(codec) = codec {
            memory(
                builder,
                row,
                result.place,
                8,
                8,
                SelectedMemoryAccessRole::WritePlace,
            )?;
            builder.emit_store(8, 8, home, codec, provenance(row))?;
        }
        memory(
            builder,
            row,
            result.place,
            0,
            8,
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(0, 8, home, value, provenance(row))?;
        builder.transport.pointers.push((result.place, home));
        Ok(())
    }

    pub(crate) fn load(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: semantic_vocabulary::PlaceId,
        home: selected_instructions::VirtualRegisterId,
    ) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
        load_pointer_word(
            builder,
            place,
            home,
            0,
            crate::selection::construction::scalar_graph::call_arguments::invocation_memory_origin(
                row,
            ),
            provenance(row),
        )
    }

    pub(crate) fn load_pointer_word(
        builder: &mut Builder<'_>,
        place: semantic_vocabulary::PlaceId,
        home: selected_instructions::VirtualRegisterId,
        byte_offset: u32,
        origin: selected_instructions::SelectedMemoryAccessOrigin,
        provenance: selected_instructions::SelectedInstructionProvenance,
    ) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
        let (address, displacement) = super::displaced_memory_pointer(
            builder,
            place,
            home,
            byte_offset,
            8,
            provenance.clone(),
        )?;
        let pointer = transport_register(builder, place, byte_offset)?;
        super::memory_at_origin(
            builder,
            origin,
            place,
            byte_offset,
            8,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        builder.emit_load64(displacement, address, pointer, provenance)?;
        Ok(pointer)
    }

    pub(crate) fn load_scalar_referent(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: semantic_vocabulary::PlaceId,
        descriptor: selected_instructions::VirtualRegisterId,
        reference: Option<&crate::selection::reference_input::BorrowedReferenceAddress>,
    ) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
        let (byte_offset, indices) = match reference {
            Some(reference) => {
                let slot = reference
                    .pointer_slots
                    .last()
                    .filter(|slot| slot.scalar_descriptor)
                    .ok_or_else(invalid)?;
                (
                    slot.byte_offset,
                    reference
                        .indices
                        .get(slot.indices.clone())
                        .ok_or_else(invalid)?,
                )
            }
            None => (0, &[][..]),
        };
        let pointer = transport_register(builder, place, byte_offset)?;
        footprint(
            builder,
            row,
            place,
            byte_offset,
            8,
            indices,
            super::runtime_address::Written::None,
        )?;
        builder.emit_load64(0, descriptor, pointer, provenance(row))?;
        Ok(pointer)
    }

    pub(crate) fn borrowed_argument(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: semantic_vocabulary::PlaceId,
        address: &crate::selection::reference_input::BorrowedReferenceAddress,
    ) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
        let pointer = super::place_pointer(builder, row, place, 0)?;
        let (slots, scalar_slot) = match address.pointer_slots.split_last() {
            Some((last, preceding)) if last.scalar_descriptor => (preceding, Some(last)),
            _ => (address.pointer_slots.as_slice(), None),
        };
        let (pointer, leaf_index) =
            cross_references(builder, row, place, pointer, &address.indices, slots)?;
        let (pointer, offset) = if let Some(slot) = scalar_slot {
            if address.referent_offset != 0 || slot.indices.end != address.indices.len() {
                return Err(invalid());
            }
            (
                scale(
                    builder,
                    row,
                    place,
                    slot.byte_offset,
                    pointer,
                    address.indices.get(slot.indices.clone()).ok_or(invalid())?,
                )?,
                slot.byte_offset,
            )
        } else {
            (
                scale(
                    builder,
                    row,
                    place,
                    address.referent_offset,
                    pointer,
                    address.indices.get(leaf_index..).ok_or(invalid())?,
                )?,
                address.referent_offset,
            )
        };
        let output = transport_register(builder, place, offset)?;
        transport_at_byte_offset(builder, pointer, output, offset, provenance(row))?;
        Ok(output)
    }
}
pub(crate) use local_storage::descriptor as stage_descriptor;
pub(crate) mod ieee_float_classify {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, SelectedInstructionKind, SelectedMemoryAccessRole, invalid,
        local_storage, memory, provenance, transport_register,
    };
    use crate::SelectedInstructionError;
    use selected_instructions::{IeeeFloatArithmeticForm, LocalStorageSlotId, MachineSemanticKind};
    use target_operations::IeeeFloatArithmeticOperation;

    pub(crate) fn establish(
        source: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::IeeeFloatClassify {
            format,
            argument,
            result,
            layout,
            ..
        } = &row.kind
        else {
            return Err(invalid());
        };
        let signature = source.structural.as_ref().ok_or_else(invalid)?;
        let declaration =
            terminal_psi::find_structural_type(&signature.structural_types, result.structural_type)
                .ok_or_else(invalid)?;
        if !result.is_unrestricted_plain()
            || !result.qualification_establishments.is_empty()
            || target_operations::conventional_float_class_layout(declaration).as_ref()
                != Some(layout)
        {
            return Err(invalid());
        }
        let (_, input, _, scalar) = builder.resolve(*argument).ok_or_else(invalid)?;
        if scalar != semantic_vocabulary::ScalarType::IeeeFloat(*format) {
            return Err(invalid());
        }
        let packed = transport_register(builder, result.place, 0)?;
        let form = IeeeFloatArithmeticForm {
            operation: IeeeFloatArithmeticOperation::Classify,
            format: *format,
        };
        let constraint = builder
            .constraints
            .keys
            .for_semantic(MachineSemanticKind::NearestIeeeFloatArithmetic(form))
            .ok_or_else(invalid)?;
        let mut computation = provenance(row);
        computation.values.push(*argument);
        computation.work = row.work.clone();
        builder.emit(
            SelectedInstructionKind::NearestIeeeFloatArithmetic { form },
            constraint,
            &[input, packed],
            computation,
        )?;
        let slot = LocalStorageSlotId::Structural {
            operation: row.operation,
            place: result.place,
        };
        if builder
            .transport
            .local_slots
            .iter()
            .any(|existing| existing.id == slot)
        {
            return Err(invalid());
        }
        builder.transport.push_structural_local_slot(
            row.operation,
            result.place,
            layout.shape.byte_size,
            layout.shape.alignment,
        );
        let pointer = local_storage::address(builder, row, slot, 0, layout.shape.byte_size, false)?;
        memory(
            builder,
            row,
            result.place,
            0,
            layout.shape.byte_size,
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(0, 8, pointer, packed, provenance(row))?;
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }
}
pub(crate) mod scalar_array {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        SelectedInstructionProvenance, SelectedMemoryAccessRole, memory,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::local_storage;
    use crate::selection::construction::scalar_graph::structural::provenance;

    pub(crate) fn establish(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let (established, shape, stores) =
            crate::selection::scalar_array_input::storage(source, row)
                .ok_or_else(|| SelectedInstructionError::custody())?;
        let slot = builder.transport.push_structural_local_slot(
            row.operation,
            established.place,
            shape.byte_size.max(1),
            shape.alignment,
        );
        let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size.max(1), true)?;
        for (element, scalar, offset, width) in stores {
            let (_, value, _, actual) = builder
                .resolve(element)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            if actual != scalar {
                return Err(SelectedInstructionError::custody());
            }
            let (element_pointer, displacement) = super::memory_pointer(
                builder,
                row,
                established.place,
                pointer,
                offset,
                u32::from(width),
            )?;
            memory(
                builder,
                row,
                established.place,
                offset,
                u32::from(width),
                SelectedMemoryAccessRole::WritePlace,
            )?;
            builder.emit_store(
                displacement,
                width,
                element_pointer,
                value,
                SelectedInstructionProvenance {
                    values: vec![element],
                    ..provenance(row)
                },
            )?;
        }
        builder
            .transport
            .pointers
            .push((established.place, pointer));
        Ok(())
    }
}
pub(crate) mod scalar_case {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, SelectedInstructionProvenance, SelectedMemoryAccessRole,
        VirtualRegisterId, memory,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::selection::construction::scalar_graph::{IntegerValue, ValueId};

    pub(crate) fn establish(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let (ordinal, placements) = crate::selection::aggregate_result_input::fields(source, row)
            .ok_or_else(|| invalid())?;
        let LegalizedScalarInstructionKind::EstablishCase {
            result,
            fields,
            layout,
            ..
        } = &row.kind
        else {
            return Err(invalid());
        };
        let pointer = super::record::initialize_storage(row, builder, result.place, layout.shape)?;
        let tag = transport_register(builder, result.place, 0)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(ordinal as u128),
            tag,
            provenance(row),
        )?;
        store(builder, row, pointer, 0, 4, tag, Vec::new())?;
        super::record::write_fields(
            source,
            row,
            builder,
            result.place,
            result.structural_type,
            pointer,
            fields,
            &placements,
        )?;
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }

    fn store(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        pointer: VirtualRegisterId,
        offset: u32,
        width: u8,
        value: VirtualRegisterId,
        values: Vec<ValueId>,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::EstablishCase { result, .. } = &row.kind else {
            return Err(invalid());
        };
        memory(
            builder,
            row,
            result.place,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(
            offset,
            width,
            pointer,
            value,
            SelectedInstructionProvenance {
                values,
                ..provenance(row)
            },
        )
    }
}
pub(crate) mod structural_array {
    use super::Builder;
    use super::record;
    use super::{
        LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    };
    use crate::SelectedInstructionError;

    pub(crate) fn establish(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let (element_shape, offsets) =
            crate::selection::structural_array_input::elements(source, row)
                .ok_or_else(super::invalid)?;
        let LegalizedScalarInstructionKind::EstablishStructuralArray {
            result,
            elements,
            shape,
        } = &row.kind
        else {
            return Err(super::invalid());
        };
        let pointer = record::initialize_storage(row, builder, result.place, *shape)?;
        for (argument, offset) in elements.iter().zip(offsets) {
            record::write_structural_operand(
                source,
                row,
                builder,
                result.place,
                pointer,
                argument,
                offset,
                element_shape,
            )?;
        }
        builder.transport.pointers.push((result.place, pointer));
        Ok(())
    }
}
pub(crate) mod structural_local {
    use super::{Builder, LegalizedScalarFunction, LegalizedScalarInstruction};
    use crate::SelectedInstructionError;

    pub(crate) fn establish(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let (established, shape) = crate::selection::structural_local_input::storage(source, row)
            .map_err(SelectedInstructionError::custody_because)?;
        let pointer = super::record::initialize_storage(row, builder, established.place, shape)?;
        builder
            .transport
            .pointers
            .push((established.place, pointer));
        Ok(())
    }
}
pub(crate) mod zero_fill {
    use super::{
        Builder, LegalizedScalarInstruction, SelectedInstructionKind, SelectedMemoryAccessRole,
        memory, transport_register,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use selected_instructions::VirtualRegisterId;
    use semantic_vocabulary::{IntegerValue, PlaceId};

    pub(crate) use crate::selection::extent_copy::INLINE_MAX_BYTES as INLINE_HOME_MAX_BYTES;

    pub(crate) fn whole_home(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        byte_size: u32,
    ) -> Result<(), SelectedInstructionError> {
        let count = transport_register(builder, place, 0)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(byte_size)),
            count,
            provenance(row),
        )?;
        let cursor = super::byte_field_store::scratch(builder, 2)?;
        memory(
            builder,
            row,
            place,
            0,
            byte_size,
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit(
            SelectedInstructionKind::ZeroBytes,
            builder
                .constraints
                .keys
                .zero_bytes
                .ok_or(SelectedInstructionError::custody())?,
            &[pointer, count, cursor],
            provenance(row),
        )
    }
}
pub(crate) mod zeroed {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction, SelectedMemoryAccessRole,
        memory, transport_register,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::local_storage;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use selected_instructions::VirtualRegisterId;
    use semantic_vocabulary::{IntegerValue, PlaceId};

    pub(crate) fn establish(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let (established, shape) = crate::selection::zeroed_structural_input::home(source, row)
            .ok_or(SelectedInstructionError::custody())?;
        let leaf_stores = (shape.byte_size <= super::zero_fill::INLINE_HOME_MAX_BYTES)
            .then(|| crate::selection::zeroed_structural_input::storage(source, established))
            .flatten();
        let slot = builder.transport.push_structural_local_slot(
            row.operation,
            established.place,
            shape.byte_size.max(1),
            shape.alignment,
        );
        let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size.max(1), true)?;
        if shape.byte_size != 0 {
            match leaf_stores {
                Some(stores) => store_each_leaf(builder, row, established.place, pointer, &stores)?,
                None => super::zero_fill::whole_home(
                    builder,
                    row,
                    established.place,
                    pointer,
                    shape.byte_size,
                )?,
            }
        }
        builder
            .transport
            .pointers
            .push((established.place, pointer));
        Ok(())
    }

    fn store_each_leaf(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        stores: &[(u32, u8)],
    ) -> Result<(), SelectedInstructionError> {
        let zero = transport_register(builder, place, 0)?;
        builder.emit_materialize_i64(IntegerValue::Unsigned(0), zero, provenance(row))?;
        for &(offset, width) in stores {
            let (pointer, displacement) =
                super::memory_pointer(builder, row, place, pointer, offset, u32::from(width))?;
            memory(
                builder,
                row,
                place,
                offset,
                u32::from(width),
                SelectedMemoryAccessRole::WritePlace,
            )?;
            builder.emit_store(displacement, width, pointer, zero, provenance(row))?;
        }
        Ok(())
    }
}
pub(crate) use primitive_locals::{global_descriptor_table, read};
pub(crate) mod encoded_scalar {
    use super::{
        Builder, LegalizedScalarInstruction, SelectedInstructionKind, VirtualRegisterId, invalid,
        provenance, transport_register,
    };
    use crate::SelectedInstructionError;
    use semantic_vocabulary::{PlaceId, ScalarType};

    pub(crate) fn backing_address(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        byte_offset: u32,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if byte_offset == 0 {
            return Ok(pointer);
        }
        let address = transport_register(builder, place, byte_offset)?;
        super::transport_at_byte_offset(builder, pointer, address, byte_offset, provenance(row))
    }

    pub(crate) fn constant_codec(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        codec: runtime_abi::ScalarStorageCodec,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let output = transport_register(builder, place, 8)?;
        builder.emit_materialize_i64(
            semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                codec.encode().ok_or(invalid())?,
            )),
            output,
            provenance(row),
        )?;
        Ok(output)
    }

    pub(crate) fn read(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        address: VirtualRegisterId,
        codec: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let definition = row.result.ok_or(invalid())?;
        if !matches!(
            definition.scalar_type,
            ScalarType::Integer(_) | ScalarType::Boolean | ScalarType::IeeeFloat(_)
        ) {
            return Err(invalid());
        }
        let output = builder.register(
            definition.value,
            definition.definition_site,
            definition.scalar_type,
        )?;
        let cursor = transport_register(builder, place, 0)?;
        let count = transport_register(builder, place, 8)?;
        let byte = transport_register(builder, place, 0)?;
        builder.emit(
            SelectedInstructionKind::LoadEncodedScalar,
            builder
                .constraints
                .keys
                .load_encoded_scalar
                .ok_or(invalid())?,
            &[address, codec, cursor, count, byte, output],
            provenance(row),
        )?;
        Ok(output)
    }

    pub(crate) fn write(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        address: VirtualRegisterId,
        codec: VirtualRegisterId,
        value: abstract_operations::AbstractResult,
    ) -> Result<(), SelectedInstructionError> {
        let (_, value_register, _, scalar) = builder.resolve(value.value).ok_or(invalid())?;
        if scalar != value.scalar_type
            || !matches!(
                scalar,
                ScalarType::Integer(_) | ScalarType::Boolean | ScalarType::IeeeFloat(_)
            )
        {
            return Err(invalid());
        }
        let mut operation_provenance = provenance(row);
        if !operation_provenance.values.contains(&value.value) {
            operation_provenance.values.push(value.value);
        }
        let cursor = transport_register(builder, place, 0)?;
        let count = transport_register(builder, place, 8)?;
        let payload = transport_register(builder, place, 0)?;
        builder.emit(
            SelectedInstructionKind::StoreEncodedScalar,
            builder
                .constraints
                .keys
                .store_encoded_scalar
                .ok_or(invalid())?,
            &[address, codec, value_register, cursor, count, payload],
            operation_provenance,
        )?;
        Ok(())
    }
}
pub(crate) mod fragmented_scalar {
    use super::{
        Builder, LegalizedScalarInstruction, PlaceId, SelectedInstructionKind, VirtualRegisterId,
        invalid, provenance, transport_register,
    };
    use crate::SelectedInstructionError;
    use legalized_operations::LegalizedRuntimeIndexOperand;
    use selected_instructions::{LocalStorageSlotId, SelectedMemoryAccessRole};
    use semantic_vocabulary::{
        AuthoredBitFragment, AuthoredBitFragments, IntegerValue, ScalarType, ValueId,
    };

    #[derive(Clone, Copy)]
    pub(crate) enum StorageAccess<'indices> {
        Place {
            indices: &'indices [LegalizedRuntimeIndexOperand],
            byte_offset: u32,
        },
        Local(LocalStorageSlotId),
        Backing,
    }

    impl StorageAccess<'_> {
        pub(crate) fn footprint(
            self,
            builder: &mut Builder<'_>,
            row: &LegalizedScalarInstruction,
            place: PlaceId,
            offset: u32,
            width: u32,
            written: super::runtime_address::Written,
        ) -> Result<(), SelectedInstructionError> {
            match self {
                Self::Place {
                    indices,
                    byte_offset,
                } => super::runtime_address::footprint(
                    builder,
                    row,
                    place,
                    byte_offset.checked_add(offset).ok_or_else(invalid)?,
                    width,
                    indices,
                    written,
                ),
                Self::Local(slot) => super::memory(
                    builder,
                    row,
                    place,
                    offset,
                    width,
                    match written {
                        super::runtime_address::Written::None => {
                            SelectedMemoryAccessRole::ReadLocal { slot }
                        }
                        _ => SelectedMemoryAccessRole::WriteLocal { slot },
                    },
                ),
                Self::Backing => super::memory(
                    builder,
                    row,
                    place,
                    offset,
                    width,
                    match written {
                        super::runtime_address::Written::None => {
                            SelectedMemoryAccessRole::ReadBacking
                        }
                        _ => SelectedMemoryAccessRole::WriteBacking,
                    },
                ),
            }
        }
    }

    pub(crate) fn constant(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        value: u64,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let output = transport_register(builder, place, 0)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(value)),
            output,
            provenance(row),
        )?;
        Ok(output)
    }

    pub(crate) fn binary(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        kind: SelectedInstructionKind,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let constraint = match kind {
            SelectedInstructionKind::WrappingAddI64 => builder.constraints.keys.add_i64,
            SelectedInstructionKind::WrappingMultiplyI64 => builder.constraints.keys.multiply_i64,
            SelectedInstructionKind::WrappingSubtractI64
            | SelectedInstructionKind::BitwiseAndI64
            | SelectedInstructionKind::BitwiseOrI64
            | SelectedInstructionKind::BitwiseXorI64 => builder.constraints.keys.subtract_i64,
            _ => {
                return Err(SelectedInstructionError::custody_because(
                    "binary register helper requires a supported arithmetic or bitwise operation",
                ));
            }
        };
        let output = transport_register(builder, place, 0)?;
        builder.emit(kind, constraint, &[left, right, output], provenance(row))?;
        Ok(output)
    }

    pub(crate) fn shift(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        input: VirtualRegisterId,
        count: u64,
        left: bool,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if count == 0 {
            return Ok(input);
        }
        if count >= 64 {
            return Err(invalid());
        }
        let count = constant(builder, row, place, count)?;
        let output = transport_register(builder, place, 0)?;
        builder.emit(
            if left {
                SelectedInstructionKind::WrappingShiftLeftI64
            } else {
                SelectedInstructionKind::WrappingShiftRightU64
            },
            builder.constraints.keys.shift_i64,
            &[input, count, output],
            provenance(row),
        )?;
        Ok(output)
    }

    pub(crate) fn mask(width: u64) -> Result<u64, SelectedInstructionError> {
        match width {
            1..=63 => Ok((1_u64 << width) - 1),
            64 => Ok(u64::MAX),
            _ => Err(invalid()),
        }
    }

    fn container_width(fragment: &AuthoredBitFragment) -> Result<u32, SelectedInstructionError> {
        if !(8..=64).contains(&fragment.container_width_bits)
            || !fragment.container_width_bits.is_multiple_of(8)
            || fragment
                .destination_lsb
                .checked_add(fragment.width)
                .is_none_or(|end| end > fragment.container_width_bits)
            || fragment
                .source_lsb
                .checked_add(fragment.width)
                .is_none_or(|end| end > 64)
        {
            return Err(invalid());
        }
        u32::try_from(fragment.container_width_bits / 8).map_err(|_| invalid())
    }

    fn prefix_offset(
        envelope_offset: u32,
        fragments: &AuthoredBitFragments,
    ) -> Result<u32, SelectedInstructionError> {
        let (first, _) = fragments.byte_envelope().ok_or_else(invalid)?;
        envelope_offset
            .checked_sub(u32::try_from(first).map_err(|_| invalid())?)
            .ok_or_else(invalid)
    }

    fn container_offset(
        prefix: u32,
        fragment: &AuthoredBitFragment,
    ) -> Result<u32, SelectedInstructionError> {
        prefix
            .checked_add(u32::try_from(fragment.container_byte_offset).map_err(|_| invalid())?)
            .ok_or_else(invalid)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn load_container(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        offset: u32,
        width: u32,
        storage: StorageAccess<'_>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if width > 8 {
            return Err(SelectedInstructionError::custody_because(
                "scalar container exceeds its register carrier",
            ));
        }
        let mut container = constant(builder, row, place, 0)?;
        for position in 0..width {
            let byte_offset = offset.checked_add(position).ok_or_else(invalid)?;
            let (address, displacement) =
                super::memory_pointer(builder, row, place, pointer, byte_offset, 1)?;
            let byte = transport_register(builder, place, byte_offset)?;
            storage.footprint(
                builder,
                row,
                place,
                byte_offset,
                1,
                super::runtime_address::Written::None,
            )?;
            builder.emit(
                SelectedInstructionKind::Load8 {
                    byte_offset: displacement,
                },
                builder.constraints.keys.load8.ok_or_else(invalid)?,
                &[address, byte],
                provenance(row),
            )?;
            let byte = shift(builder, row, place, byte, u64::from(position) * 8, true)?;
            container = binary(
                builder,
                row,
                place,
                SelectedInstructionKind::BitwiseOrI64,
                container,
                byte,
            )?;
        }
        Ok(container)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn read(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        envelope_offset: u32,
        fragments: &AuthoredBitFragments,
        indices: &[LegalizedRuntimeIndexOperand],
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let definition = row.result.ok_or_else(invalid)?;
        if !matches!(
            definition.scalar_type,
            ScalarType::Boolean | ScalarType::Integer(_)
        ) {
            return Err(invalid());
        }
        let value = read_register(
            builder,
            row,
            place,
            pointer,
            envelope_offset,
            fragments,
            StorageAccess::Place {
                indices,
                byte_offset: 0,
            },
        )?;
        let output = builder.register(
            definition.value,
            definition.definition_site,
            definition.scalar_type,
        )?;
        builder.emit(
            crate::selection::scalar_call_abi::integer_carrier_normalization(
                definition.scalar_type,
            ),
            builder.constraints.keys.copy_i64,
            &[value, output],
            provenance(row),
        )?;
        Ok(output)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn read_register(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        envelope_offset: u32,
        fragments: &AuthoredBitFragments,
        storage: StorageAccess<'_>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let prefix = prefix_offset(envelope_offset, fragments)?;
        let mut value = constant(builder, row, place, 0)?;
        for fragment in fragments.fragments() {
            let width = container_width(fragment)?;
            let offset = container_offset(prefix, fragment)?;
            let container = load_container(builder, row, place, pointer, offset, width, storage)?;
            let payload = shift(
                builder,
                row,
                place,
                container,
                fragment.destination_lsb,
                false,
            )?;
            let source_mask = constant(builder, row, place, mask(fragment.width)?)?;
            let payload = binary(
                builder,
                row,
                place,
                SelectedInstructionKind::BitwiseAndI64,
                payload,
                source_mask,
            )?;
            let payload = shift(builder, row, place, payload, fragment.source_lsb, true)?;
            value = binary(
                builder,
                row,
                place,
                SelectedInstructionKind::BitwiseOrI64,
                value,
                payload,
            )?;
        }
        Ok(value)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        envelope_offset: u32,
        fragments: &AuthoredBitFragments,
        indices: &[LegalizedRuntimeIndexOperand],
        value: ValueId,
    ) -> Result<(), SelectedInstructionError> {
        let (_, value_register, _, scalar) = builder.resolve(value).ok_or_else(invalid)?;
        if !matches!(scalar, ScalarType::Boolean | ScalarType::Integer(_)) {
            return Err(invalid());
        }
        write_register(
            builder,
            row,
            place,
            pointer,
            envelope_offset,
            fragments,
            StorageAccess::Place {
                indices,
                byte_offset: 0,
            },
            value_register,
            Some(value),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_register(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        envelope_offset: u32,
        fragments: &AuthoredBitFragments,
        storage: StorageAccess<'_>,
        value_register: VirtualRegisterId,
        value: Option<ValueId>,
    ) -> Result<(), SelectedInstructionError> {
        let prefix = prefix_offset(envelope_offset, fragments)?;
        for fragment in fragments.fragments() {
            let width = container_width(fragment)?;
            let offset = container_offset(prefix, fragment)?;
            let container = load_container(builder, row, place, pointer, offset, width, storage)?;
            let source_mask_bits = mask(fragment.width)?;
            let destination_mask = source_mask_bits
                .checked_shl(u32::try_from(fragment.destination_lsb).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            let keep = constant(builder, row, place, !destination_mask)?;
            let preserved = binary(
                builder,
                row,
                place,
                SelectedInstructionKind::BitwiseAndI64,
                container,
                keep,
            )?;
            let payload = shift(
                builder,
                row,
                place,
                value_register,
                fragment.source_lsb,
                false,
            )?;
            let source_mask = constant(builder, row, place, source_mask_bits)?;
            let payload = binary(
                builder,
                row,
                place,
                SelectedInstructionKind::BitwiseAndI64,
                payload,
                source_mask,
            )?;
            let payload = shift(builder, row, place, payload, fragment.destination_lsb, true)?;
            let merged = binary(
                builder,
                row,
                place,
                SelectedInstructionKind::BitwiseOrI64,
                preserved,
                payload,
            )?;
            store_container(
                builder, row, place, pointer, offset, width, storage, merged, value,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn store_container(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        pointer: VirtualRegisterId,
        offset: u32,
        width: u32,
        storage: StorageAccess<'_>,
        bits: VirtualRegisterId,
        value: Option<ValueId>,
    ) -> Result<(), SelectedInstructionError> {
        if width > 8 {
            return Err(SelectedInstructionError::custody_because(
                "scalar container exceeds its register carrier",
            ));
        }
        for position in 0..width {
            let byte_offset = offset.checked_add(position).ok_or_else(invalid)?;
            let payload = shift(builder, row, place, bits, u64::from(position) * 8, false)?;
            let (address, displacement) =
                super::memory_pointer(builder, row, place, pointer, byte_offset, 1)?;
            storage.footprint(
                builder,
                row,
                place,
                byte_offset,
                1,
                value.map_or(
                    super::runtime_address::Written::Structural,
                    super::runtime_address::Written::Scalar,
                ),
            )?;
            let mut operation_provenance = provenance(row);
            if let Some(value) = value
                && !operation_provenance.values.contains(&value)
            {
                operation_provenance.values.push(value);
            }
            builder.emit_store(displacement, 1, address, payload, operation_provenance)?;
        }
        Ok(())
    }
}
pub(crate) mod runtime_address {
    use super::{
        Builder, IntegerSign, SelectedInstructionKind, SelectedInstructionProvenance,
        SelectedMemoryAccessRole, VirtualRegisterId, memory_at_origin, transport_register,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::established_view_input::{element_view_type, view_type};
    use crate::structural_inputs::structural_reference_input::PointerSlot;
    use legalized_operations::LegalizedRuntimeIndexOperand;
    use semantic_vocabulary::{IntegerValue, PlaceId, ValueId};

    pub(crate) fn projection_base(
        source: &super::LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: PlaceId,
        byte_offset: u32,
    ) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
        let byte_view = view_type(source, place).is_some();
        let view_offset = if byte_view {
            builder.transport.view(place).map(|view| view.byte_offset)
        } else {
            builder
                .transport
                .element_view(place)
                .map(|view| view.byte_offset)
        };
        if !byte_view && view_offset.is_none() && element_view_type(source, place).is_none() {
            return Ok(builder
                .transport
                .pointers
                .iter()
                .find(|(stored, _)| *stored == place)
                .map(|(_, pointer)| *pointer));
        }
        let backing = if byte_view {
            super::byte_views::backing_pointer(source, builder, row, place)?
        } else {
            super::element_views::element_backing_pointer(builder, row, place)?
        };
        let Some(view_offset) = view_offset else {
            return Ok(Some(backing));
        };
        let base = transport_register(builder, place, byte_offset)?;
        builder.emit_byte_view_address(backing, view_offset, base, super::provenance(row))?;
        Ok(Some(base))
    }

    pub(crate) fn projection_origin<'slots>(
        source: &super::LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: PlaceId,
        byte_offset: u32,
        mut slots: &'slots [PointerSlot],
    ) -> Result<(Option<VirtualRegisterId>, &'slots [PointerSlot]), SelectedInstructionError> {
        let root_view =
            view_type(source, place).is_some() || element_view_type(source, place).is_some();
        let pointer = if root_view {
            if let Some(first) = slots.first() {
                if first.byte_offset != 0 || first.indices != (0..0) || first.scalar_descriptor {
                    return Err(invalid());
                }
                slots = &slots[1..];
            }
            projection_base(source, builder, row, place, byte_offset)?
        } else if !slots.is_empty() {
            Some(super::place_pointer(builder, row, place, 0)?)
        } else {
            projection_base(source, builder, row, place, byte_offset)?
        };
        Ok((pointer, slots))
    }

    pub(crate) fn scale(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: PlaceId,
        byte_offset: u32,
        address: VirtualRegisterId,
        indices: &[LegalizedRuntimeIndexOperand],
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        scale_at_site(
            builder,
            super::provenance(row),
            place,
            byte_offset,
            address,
            indices,
        )
    }

    pub(crate) fn scale_at_site(
        builder: &mut Builder<'_>,
        provenance: SelectedInstructionProvenance,
        place: PlaceId,
        byte_offset: u32,
        mut address: VirtualRegisterId,
        indices: &[LegalizedRuntimeIndexOperand],
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        for index in indices {
            let (_, index_register, _, index_type) = builder
                .resolve(index.operand.value)
                .ok_or_else(|| invalid())?;
            let Some(integer) = index.operand.scalar_type.as_integer() else {
                return Err(invalid());
            };
            if integer.bits() > 64 || index_type != index.operand.scalar_type {
                return Err(invalid());
            }
            let index_register = match integer.bits() {
                64 => index_register,
                bits => {
                    let kind = match (integer.sign(), bits) {
                        (IntegerSign::Unsigned, 8) => SelectedInstructionKind::ZeroExtendU8,
                        (IntegerSign::Unsigned, 16) => SelectedInstructionKind::ZeroExtendU16,
                        (IntegerSign::Unsigned, 32) => SelectedInstructionKind::ZeroExtendU32,
                        (IntegerSign::Signed, 8) => SelectedInstructionKind::SignExtendI8,
                        (IntegerSign::Signed, 16) => SelectedInstructionKind::SignExtendI16,
                        (IntegerSign::Signed, 32) => SelectedInstructionKind::SignExtendI32,
                        _ => return Err(invalid()),
                    };
                    let extended = transport_register(builder, place, byte_offset)?;
                    builder.emit(
                        kind,
                        builder.constraints.keys.copy_i64,
                        &[index_register, extended],
                        SelectedInstructionProvenance {
                            values: vec![index.operand.value],
                            ..provenance.clone()
                        },
                    )?;
                    extended
                }
            };
            let stride = transport_register(builder, place, byte_offset)?;
            builder.emit_materialize_i64(
                IntegerValue::Unsigned(u128::from(index.stride)),
                stride,
                provenance.clone(),
            )?;
            let scaled = transport_register(builder, place, byte_offset)?;
            builder.emit(
                SelectedInstructionKind::WrappingMultiplyI64,
                builder.constraints.keys.multiply_i64,
                &[index_register, stride, scaled],
                SelectedInstructionProvenance {
                    values: vec![index.operand.value],
                    ..provenance.clone()
                },
            )?;
            let joined = transport_register(builder, place, byte_offset)?;
            builder.emit_byte_view_address(
                address,
                scaled,
                joined,
                SelectedInstructionProvenance {
                    values: vec![index.operand.value],
                    obligations: vec![index.obligation],
                    ..provenance.clone()
                },
            )?;
            address = joined;
        }
        Ok(address)
    }

    pub(crate) enum Written {
        None,
        Scalar(ValueId),
        Structural,
    }

    pub(crate) fn footprint(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: PlaceId,
        byte_offset: u32,
        byte_count: u32,
        indices: &[LegalizedRuntimeIndexOperand],
        written: Written,
    ) -> Result<(), SelectedInstructionError> {
        footprint_at_origin(
            builder,
            crate::selection::construction::scalar_graph::call_arguments::invocation_memory_origin(
                row,
            ),
            place,
            byte_offset,
            byte_count,
            indices,
            written,
        )
    }

    pub(crate) fn footprint_at_origin(
        builder: &mut Builder<'_>,
        origin: selected_instructions::SelectedMemoryAccessOrigin,
        place: PlaceId,
        byte_offset: u32,
        byte_count: u32,
        indices: &[LegalizedRuntimeIndexOperand],
        written: Written,
    ) -> Result<(), SelectedInstructionError> {
        if indices.is_empty() {
            let role = if matches!(written, Written::None) {
                SelectedMemoryAccessRole::ReadPlace
            } else {
                SelectedMemoryAccessRole::WritePlace
            };
            return memory_at_origin(builder, origin, place, byte_offset, byte_count, role);
        }
        for index in indices {
            let role = match written {
                Written::Scalar(value) => SelectedMemoryAccessRole::WriteIndexedPrimitive {
                    index: index.operand.value,
                    value,
                    stride: index.stride,
                    extent: index.extent,
                    obligation: index.obligation,
                    accepted_fact: index.accepted_fact,
                },
                Written::Structural => SelectedMemoryAccessRole::WriteIndexedStructural {
                    index: index.operand.value,
                    stride: index.stride,
                    extent: index.extent,
                    obligation: index.obligation,
                    accepted_fact: index.accepted_fact,
                },
                Written::None => SelectedMemoryAccessRole::ReadIndexedPrimitive {
                    index: index.operand.value,
                    stride: index.stride,
                    extent: index.extent,
                    obligation: index.obligation,
                    accepted_fact: index.accepted_fact,
                },
            };
            memory_at_origin(builder, origin, place, byte_offset, byte_count, role)?;
        }
        Ok(())
    }

    pub(crate) fn cross_references(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: PlaceId,
        pointer: VirtualRegisterId,
        indices: &[LegalizedRuntimeIndexOperand],
        pointer_slots: &[PointerSlot],
    ) -> Result<(VirtualRegisterId, usize), SelectedInstructionError> {
        cross_references_at_site(
            builder,
            super::provenance(row),
            crate::selection::construction::scalar_graph::call_arguments::invocation_memory_origin(
                row,
            ),
            place,
            pointer,
            indices,
            pointer_slots,
        )
    }

    pub(crate) fn cross_references_at_site(
        builder: &mut Builder<'_>,
        provenance: SelectedInstructionProvenance,
        origin: selected_instructions::SelectedMemoryAccessOrigin,
        place: PlaceId,
        mut pointer: VirtualRegisterId,
        indices: &[LegalizedRuntimeIndexOperand],
        pointer_slots: &[PointerSlot],
    ) -> Result<(VirtualRegisterId, usize), SelectedInstructionError> {
        let mut leaf_index = 0;
        for slot in pointer_slots {
            if slot.scalar_descriptor {
                return Err(invalid());
            }
            let slot_indices = indices.get(slot.indices.clone()).ok_or(invalid())?;
            let address = scale_at_site(
                builder,
                provenance.clone(),
                place,
                slot.byte_offset,
                pointer,
                slot_indices,
            )?;
            let (address, displacement) = super::displaced_memory_pointer(
                builder,
                place,
                address,
                slot.byte_offset,
                8,
                provenance.clone(),
            )?;
            footprint_at_origin(
                builder,
                origin,
                place,
                slot.byte_offset,
                8,
                slot_indices,
                Written::None,
            )?;
            let loaded = super::transport_register(builder, place, slot.byte_offset)?;
            builder.emit_load64(displacement, address, loaded, provenance.clone())?;
            pointer = loaded;
            leaf_index = slot.indices.end;
        }
        Ok((pointer, leaf_index))
    }

    pub(crate) fn scalar_reference(
        source: &super::LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        place: PlaceId,
        pointer: VirtualRegisterId,
        indices: &[LegalizedRuntimeIndexOperand],
        slots: &[PointerSlot],
    ) -> Result<Option<(VirtualRegisterId, VirtualRegisterId)>, SelectedInstructionError> {
        let (home, byte_offset, traversals) = if let Some(slot) =
            slots.last().filter(|slot| slot.scalar_descriptor)
        {
            let (pointer, _) = cross_references(
                builder,
                row,
                place,
                pointer,
                indices,
                &slots[..slots.len() - 1],
            )?;
            let traversals = indices.get(slot.indices.clone()).ok_or(invalid())?;
            let home = scale(builder, row, place, slot.byte_offset, pointer, traversals)?;
            (home, slot.byte_offset, traversals)
        } else if crate::selection::reference_input::borrowed_scalar_source(source, place).is_some()
        {
            (pointer, 0, &[][..])
        } else {
            return Ok(None);
        };
        let (home, displacement) =
            super::memory_pointer(builder, row, place, home, byte_offset, 16)?;
        let address = transport_register(builder, place, byte_offset)?;
        footprint(
            builder,
            row,
            place,
            byte_offset,
            8,
            traversals,
            Written::None,
        )?;
        builder.emit_load64(displacement, home, address, super::provenance(row))?;
        let codec_offset = byte_offset.checked_add(8).ok_or(invalid())?;
        let codec = transport_register(builder, place, codec_offset)?;
        footprint(
            builder,
            row,
            place,
            codec_offset,
            8,
            traversals,
            Written::None,
        )?;
        builder.emit_load64(
            displacement.checked_add(8).ok_or(invalid())?,
            home,
            codec,
            super::provenance(row),
        )?;
        Ok(Some((address, codec)))
    }
}
pub(crate) mod scalar_store {
    use super::{
        Builder, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use crate::structural_inputs::structural_reference_input::runtime_indices_match;
    use crate::structural_inputs::structural_reference_input::{
        field_store_geometry, primitive_store, projected_scalar_codec, scalar_field_codec,
        scalar_field_fragments,
    };

    pub(crate) fn emit(
        source: &LegalizedScalarFunction,
        row: &LegalizedScalarInstruction,
        builder: &mut Builder<'_>,
    ) -> Result<(), SelectedInstructionError> {
        let (
            destination,
            value,
            byte_offset,
            byte_size,
            indices,
            reference_destination,
            pointer_slots,
        ) = match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                byte_offset,
                byte_size,
                indices,
            } => {
                let signature = source.structural.as_ref().ok_or_else(|| invalid())?;
                let reference = crate::selection::reference_input::store_destination(
                    source,
                    destination,
                    path,
                    terminal_psi::StructuralAccess::WriteOnlyBorrow,
                );
                let geometry = field_store_geometry(
                    destination.structural_type(),
                    path,
                    *field,
                    value.scalar_type,
                    &signature.structural_types,
                )
                .ok_or_else(|| invalid())?;
                if (geometry.byte_offset, geometry.byte_size) != (*byte_offset, *byte_size)
                    || !runtime_indices_match(indices, &geometry.elements)
                    || path.contains(&terminal_psi::StructuralPathSegment::Referent)
                        && reference.is_none()
                {
                    return Err(invalid());
                }
                (
                    destination.clone(),
                    value,
                    *byte_offset,
                    *byte_size,
                    indices,
                    reference.is_some(),
                    geometry.pointer_slots,
                )
            }
            LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                destination,
                path,
                value,
                byte_offset,
                byte_size,
                indices,
            } => {
                let signature = source.structural.as_ref().ok_or_else(|| invalid())?;
                let geometry = primitive_store(
                    destination,
                    path,
                    value.scalar_type,
                    &signature.structural_types,
                )
                .ok_or_else(|| invalid())?;
                if (geometry.byte_offset, geometry.byte_size)
                    != (*byte_offset, u32::from(*byte_size))
                    || !runtime_indices_match(indices, &geometry.elements)
                {
                    return Err(invalid());
                }
                (
                    destination.clone(),
                    value,
                    *byte_offset,
                    u32::from(*byte_size),
                    indices,
                    false,
                    geometry.pointer_slots,
                )
            }
            _ => return Err(invalid()),
        };
        let declared = match &destination {
            abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
                crate::selection::arrival_parameter_input::store_destination(source, parameter)
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                producer,
                result,
            } => {
                reference_destination
                    || crate::selection::aggregate_result_input::owned_store_destination(
                        source, *producer, result,
                    )
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                block,
                parameter,
            } => crate::selection::owned_block_home_input::store_destination(
                source, *block, parameter,
            ),
        };
        if row.result.is_some() || !declared {
            return Err(invalid());
        }
        let (pointer, pointer_slots) = super::runtime_address::projection_origin(
            source,
            builder,
            row,
            destination.place(),
            byte_offset,
            &pointer_slots,
        )?;
        let pointer = pointer.ok_or_else(invalid)?;
        if let Some((address, codec)) = super::runtime_address::scalar_reference(
            source,
            builder,
            row,
            destination.place(),
            pointer,
            indices,
            pointer_slots,
        )? {
            if byte_offset != 0 {
                return Err(invalid());
            }
            return super::encoded_scalar::write(
                builder,
                row,
                destination.place(),
                address,
                codec,
                *value,
            );
        }
        let (pointer, leaf_index) = super::runtime_address::cross_references(
            builder,
            row,
            destination.place(),
            pointer,
            indices,
            pointer_slots,
        )?;
        let leaf_indices = &indices[leaf_index..];
        let (_, register, _, scalar) = builder.resolve(value.value).ok_or_else(|| invalid())?;
        if scalar != value.scalar_type {
            return Err(invalid());
        }
        let address = super::runtime_address::scale(
            builder,
            row,
            destination.place(),
            byte_offset,
            pointer,
            leaf_indices,
        )?;
        let declarations = &source
            .structural
            .as_ref()
            .ok_or_else(|| invalid())?
            .structural_types;
        if let LegalizedScalarInstructionKind::StructuralScalarFieldStore { path, field, .. } =
            &row.kind
            && let Some(fragments) =
                scalar_field_fragments(destination.structural_type(), path, *field, declarations)
        {
            if matches!(&destination,
                    abstract_operations::StructuralScalarStoreDestination::Parameter(parameter)
                        if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow)
                || reference_destination
                    && crate::selection::reference_input::referent_projection(
                        source,
                        destination.place(),
                        path,
                        terminal_psi::StructuralAccess::MutableBorrow,
                    )
                    .is_none()
            {
                return Err(invalid());
            }
            return super::fragmented_scalar::write(
                builder,
                row,
                destination.place(),
                address,
                byte_offset,
                fragments,
                leaf_indices,
                value.value,
            );
        }
        let codec = match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldStore { path, field, .. } => {
                scalar_field_codec(destination.structural_type(), path, *field, declarations)
            }
            LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { path, .. } => {
                projected_scalar_codec(destination.structural_type(), path, declarations)
            }
            _ => None,
        }
        .ok_or_else(|| invalid())?;
        if u32::from(codec.byte_size()) != byte_size {
            return Err(invalid());
        }
        let canonical = runtime_abi::ScalarStorageCodec::from_scalar(value.scalar_type)
            .ok_or_else(|| invalid())?;
        if codec != canonical || matches!(codec.byte_size(), 3 | 5 | 6 | 7) {
            let address = super::encoded_scalar::backing_address(
                builder,
                row,
                destination.place(),
                address,
                byte_offset,
            )?;
            let codec =
                super::encoded_scalar::constant_codec(builder, row, destination.place(), codec)?;
            return super::encoded_scalar::write(
                builder,
                row,
                destination.place(),
                address,
                codec,
                *value,
            );
        }
        let (address, displacement) = super::memory_pointer(
            builder,
            row,
            destination.place(),
            address,
            byte_offset,
            byte_size,
        )?;
        super::runtime_address::footprint(
            builder,
            row,
            destination.place(),
            byte_offset,
            byte_size,
            leaf_indices,
            super::runtime_address::Written::Scalar(value.value),
        )?;
        builder.emit_store(
            displacement,
            u8::try_from(byte_size).map_err(|_| invalid())?,
            address,
            register,
            operation_provenance(
                row,
                indices
                    .iter()
                    .map(|index| index.operand.value)
                    .chain([value.value])
                    .collect(),
            ),
        )?;
        Ok(())
    }
}
pub(crate) mod subslice {
    use super::{
        Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
        SelectedInstructionProvenance, obligation_provenance,
    };
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::ByteViewHomes;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::transport_register;

    pub(crate) fn create(
        function: &LegalizedScalarFunction,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::ByteSequenceSubslice {
            result,
            source,
            indices,
            start,
            end,
            length,
            obligation,
            accepted_fact,
        } = &row.kind
        else {
            return Err(invalid());
        };
        if row.result.is_some()
            || super::descriptor_operands::source_geometry(function, source).map(|(_, shape)| {
                matches!(shape, terminal_psi::StructuralTypeShape::ByteSequence(_))
            }) != Some(true)
            || crate::selection::established_view_input::view_access(function, result.place)
                .is_none()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || builder
                .transport
                .pointers
                .iter()
                .any(|(place, _)| *place == result.place)
            || builder
                .transport
                .views
                .iter()
                .any(|view| view.place == result.place)
        {
            return Err(invalid());
        }
        let integer = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
        );
        let (_, start_register, _, start_type) =
            builder.resolve(*start).ok_or_else(|| invalid())?;
        let (_, end_register, _, end_type) = builder.resolve(*end).ok_or_else(|| invalid())?;
        let (_, _, _, length_type) = builder.resolve(*length).ok_or_else(|| invalid())?;
        if start_type != integer || end_type != integer || length_type != integer {
            return Err(invalid());
        }
        let parent = source
            .path
            .is_empty()
            .then(|| builder.transport.view(source.place).copied())
            .flatten();
        let backing = match parent {
            Some(parent) => parent.backing_pointer,
            None => super::descriptor_operands::backing(function, builder, row, source, indices)?,
        };
        let (byte_offset, root_length) = if let Some(parent) = parent {
            let byte_offset = transport_register(builder, result.place, 0)?;
            builder.emit(
                SelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                builder.constraints.keys.add_i64,
                &[parent.byte_offset, start_register, byte_offset],
                obligation_provenance(
                    row,
                    &[*start, *end, *length, parent.root_length],
                    &[*obligation],
                ),
            )?;
            (byte_offset, parent.root_length)
        } else {
            (start_register, *length)
        };
        let byte_length = transport_register(builder, result.place, 8)?;
        builder.emit(
            SelectedInstructionKind::ExactSubtractI64 {
                obligation: *obligation,
                accepted_fact: *accepted_fact,
            },
            builder.constraints.keys.subtract_i64,
            &[end_register, start_register, byte_length],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![*start, *end, *length],
                obligations: vec![*obligation],
                work: row.work.clone(),
                ..Default::default()
            },
        )?;
        builder.transport.views.push(ByteViewHomes {
            place: result.place,
            source: source.place,
            root: if let Some(parent) = parent {
                parent.root
            } else {
                crate::selection::byte_view_homes::own_view_root(function, source.place)
            },
            backing_pointer: backing,
            byte_offset,
            byte_length,
            root_length,
        });
        if crate::selection::established_view_input::requires_descriptor(function, result.place) {
            let slot =
                builder
                    .transport
                    .push_structural_local_slot(row.operation, result.place, 16, 8);
            let pointer = transport_register(builder, result.place, 0)?;
            builder.emit_byte_view_address(
                backing,
                byte_offset,
                pointer,
                obligation_provenance(row, &[*start, *end, *length, root_length], &[*obligation]),
            )?;
            super::local_storage::store(builder, row, slot, 0, pointer)?;
            super::local_storage::store(builder, row, slot, 8, byte_length)?;
            let descriptor = super::local_storage::address(builder, row, slot, 0, 16, false)?;
            builder.transport.pointers.push((result.place, descriptor));
        }
        Ok(())
    }
}

pub(crate) use byte_field_read::read_byte as byte_field_read;
pub(crate) use byte_views::{byte_equal, byte_observation, byte_predicate};
pub(crate) use element_views::element_observation;

#[derive(Default)]
pub(crate) struct Transport {
    pub(crate) result_pointer: Option<VirtualRegisterId>,
    pub pointers: Vec<(PlaceId, VirtualRegisterId)>,
    pub(crate) views: Vec<ByteViewHomes>,
    pub(crate) element_views: Vec<ElementViewHomes>,
    pub fragments: Vec<(PlaceId, u32, VirtualRegisterId)>,
    pub slots: Vec<SelectedOutgoingArgumentSlot>,
    pub local_slots: Vec<selected_instructions::SelectedLocalStorageSlot>,
    pub calls: Vec<SelectedCallContract>,
    pub normalized_foreign_calls: Vec<selected_instructions::SelectedNormalizedForeignCall>,
    pub memory: Vec<SelectedMemoryAccess>,
    pub settlements: Vec<SelectedBoundarySettlement>,
}

impl Transport {
    pub(crate) fn push_structural_local_slot(
        &mut self,
        operation: OperationId,
        place: PlaceId,
        byte_size: u32,
        alignment: u16,
    ) -> selected_instructions::LocalStorageSlotId {
        let id = selected_instructions::LocalStorageSlotId::Structural { operation, place };
        self.local_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id,
                byte_size,
                alignment,
            });
        id
    }

    pub(crate) fn view(&self, place: PlaceId) -> Option<&ByteViewHomes> {
        self.views.iter().find(|view| view.place == place)
    }

    pub(crate) fn element_view(&self, place: PlaceId) -> Option<&ElementViewHomes> {
        self.element_views.iter().find(|view| view.place == place)
    }
}

#[track_caller]
pub(crate) fn invalid() -> SelectedInstructionError {
    SelectedInstructionError::custody()
}

pub(crate) fn transport_register(
    builder: &mut Builder<'_>,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    crate::selection::construction::scalar_graph::structural_case::register(
        builder,
        place,
        byte_offset,
        64,
        false,
    )
}

pub(crate) fn scaled_element_index(
    builder: &mut Builder<'_>,
    place: PlaceId,
    stride: u32,
    index_register: VirtualRegisterId,
    index: ValueId,
    length: ValueId,
    obligation: ObligationId,
    accepted_fact: AcceptedObligationFactIdentity,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let view = builder.transport.element_view(place).copied();
    let stride_register = transport_register(builder, place, 0)?;
    let scaled_index = transport_register(builder, place, 0)?;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(u128::from(stride)),
        stride_register,
        obligation_provenance(row, &[index, length], &[obligation]),
    )?;
    builder.emit(
        SelectedInstructionKind::WrappingMultiplyI64,
        builder.constraints.keys.multiply_i64,
        &[index_register, stride_register, scaled_index],
        obligation_provenance(row, &[index, length], &[obligation]),
    )?;
    let physical_index = if let Some(view) = view {
        let offset = transport_register(builder, place, 0)?;
        builder.emit(
            SelectedInstructionKind::ExactAddI64 {
                obligation,
                accepted_fact,
            },
            builder.constraints.keys.add_i64,
            &[view.byte_offset, scaled_index, offset],
            obligation_provenance(row, &[index, length, view.root_length], &[obligation]),
        )?;
        offset
    } else {
        scaled_index
    };
    Ok(physical_index)
}

pub(crate) fn place_pointer(
    builder: &mut Builder<'_>,
    row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    place_pointer_at_site(builder, invocation_provenance(row), place, byte_offset)
}

pub(crate) fn place_pointer_at_site(
    builder: &mut Builder<'_>,
    provenance: SelectedInstructionProvenance,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let incoming = builder
        .transport
        .pointers
        .iter()
        .find(|(source, _)| *source == place)
        .map(|(_, pointer)| *pointer);
    let input = if let Some(pointer) = incoming {
        pointer
    } else {
        let mut homes = builder
            .transport
            .local_slots
            .iter()
            .filter(|home| home.id.structural_place() == Some(place));
        let home = homes
            .next()
            .ok_or_else(|| {
                SelectedInstructionError::custody_because(
                    "address transport has neither an incoming pointer nor a local aggregate home",
                )
            })?
            .clone();
        if homes.next().is_some() {
            return Err(invalid());
        }
        let pointer = transport_register(builder, place, 0)?;
        let origin = match (
            provenance.operations.as_slice(),
            provenance.edges.as_slice(),
        ) {
            ([operation], []) => {
                selected_instructions::SelectedMemoryAccessOrigin::Operation(*operation)
            }
            ([], [edge]) => selected_instructions::SelectedMemoryAccessOrigin::Edge(*edge),
            _ => return Err(invalid()),
        };
        memory_at_origin(
            builder,
            origin,
            place,
            0,
            home.byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot: home.id },
        )?;
        builder.emit_local_frame_address(home.id, pointer, provenance.clone())?;
        pointer
    };
    let output = transport_register(builder, place, byte_offset)?;
    transport_at_byte_offset(builder, input, output, byte_offset, provenance)
}

pub(crate) fn view_element_pointer(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let input = runtime_address::projection_base(source, builder, row, place, byte_offset)?
        .ok_or_else(|| invalid())?;
    let output = transport_register(builder, place, byte_offset)?;
    transport_at_byte_offset(
        builder,
        input,
        output,
        byte_offset,
        invocation_provenance(row),
    )
}

macro_rules! dispatch_kind {
    ($row:expr, $pattern:pat, $emit:expr) => {
        if matches!($row.kind, $pattern) {
            $emit?;
            return Ok(true);
        }
    };
}

pub(crate) fn operation(
    function: usize,
    source: &LegalizedScalarFunction,
    block: SelectedBlockId,
    block_start: usize,
    row: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<bool, SelectedInstructionError> {
    use LegalizedScalarInstructionKind as Kind;
    dispatch_kind!(
        row,
        Kind::InstallStaticInterrupt { .. },
        interrupt_installation::emit(function, source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::IeeeFloatClassify { .. },
        ieee_float_classify::establish(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::EstablishByteSequenceConcatenation { .. },
        byte_concatenation::establish(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::StructuralByteSequenceFieldByteStore { .. },
        byte_field_store::replace_byte(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::ByteSequenceStore { .. } | Kind::StructuralByteSequenceFieldStore { .. },
        byte_field_store::replace(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::StructuralByteViewFieldStore { .. },
        byte_view_field_store::replace(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::ByteSequenceWrite { .. },
        byte_views::write(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::ElementViewWrite { .. },
        element_view_write::write(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::EstablishPrimitiveLocal { .. } | Kind::PrimitiveLocalStore { .. },
        primitive_locals::write(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::EstablishRecord { .. },
        record::establish(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::StaticInterrupt { .. },
        static_interrupts::emit(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::AccessLifecycle { .. },
        access_lifecycles::emit(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::EstablishCase { .. },
        scalar_case::establish(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::StructuralLeafCopy { .. },
        leaf_copy::copy(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::StoreStructuralField { .. },
        leaf_copy::reseat(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::EstablishStructuralArray { .. },
        structural_array::establish(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::EstablishScalarArray { .. },
        scalar_array::establish(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::EstablishZeroedStructural { .. },
        zeroed::establish(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::EstablishStructuralLocal { .. },
        structural_local::establish(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::StructuralScalarFieldStore { .. } | Kind::WriteOnlyPrimitiveStore { .. },
        scalar_store::emit(source, row, builder)
    );
    dispatch_kind!(
        row,
        Kind::EstablishActivationClaim { .. }
            | Kind::SplitActivationClaim { .. }
            | Kind::MergeActivationClaim { .. }
            | Kind::ReleaseActivationClaim { .. },
        activation_storage::emit(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::EstablishNativeCodeLayout { .. },
        native_code_layout::establish(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::EstablishByteSequenceLiteral { .. },
        literals::establish(builder, row)
    );
    dispatch_kind!(
        row,
        Kind::ByteSequenceSubslice { .. },
        subslice::create(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::EstablishElementView { .. },
        element_establishment::establish(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::ElementViewSubslice { .. },
        element_subslice::create(source, builder, row)
    );
    dispatch_kind!(
        row,
        Kind::EstablishReference { .. } | Kind::EstablishAccessReference { .. },
        reference::establish(source, row, builder)
    );
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::ReleaseReference { .. }
            | LegalizedScalarInstructionKind::EstablishTrivialAffineLocal { .. }
    ) {
        if row.result.is_some() {
            return Err(invalid());
        }
        return Ok(true);
    }
    if let LegalizedScalarInstructionKind::BoundarySettlement(settlement) = &row.kind {
        if row.result.is_some()
            || row.operation != settlement.operation
            || row.effect != settlement.effect
            || row.ownership != settlement.ownership
            || row.work != settlement.work
        {
            return Err(invalid());
        }
        let instruction_index = builder
            .instructions
            .len()
            .checked_sub(block_start)
            .ok_or_else(|| invalid())?;
        builder
            .transport
            .settlements
            .push(SelectedBoundarySettlement {
                block,
                instruction_index: instruction_index.try_into().map_err(|_| invalid())?,
                settlement:
                    selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(
                        settlement.clone(),
                    ),
            });
        return Ok(true);
    }
    dispatch_kind!(
        row,
        Kind::NormalizedForeignCall(_),
        crate::selection::construction::scalar_graph::normalized_foreign::emit(
            function,
            source,
            row,
            environment,
            builder
        )
    );
    if row.result.is_some() {
        return Ok(false);
    }
    if !matches!(row.kind, LegalizedScalarInstructionKind::Call(_)) {
        return Err(invalid());
    }
    crate::selection::construction::scalar_graph::unit_call::emit(
        function,
        source,
        row,
        environment,
        builder,
    )?;
    Ok(true)
}

pub(crate) fn provenance(
    row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
) -> SelectedInstructionProvenance {
    invocation_provenance(row)
}

pub(crate) fn operation_provenance(
    row: &LegalizedScalarInstruction,
    values: Vec<ValueId>,
) -> SelectedInstructionProvenance {
    SelectedInstructionProvenance {
        operations: vec![row.operation],
        values,
        work: row.work.clone(),
        ..Default::default()
    }
}

pub(crate) fn obligation_provenance(
    row: &LegalizedScalarInstruction,
    values: &[ValueId],
    obligations: &[ObligationId],
) -> SelectedInstructionProvenance {
    SelectedInstructionProvenance {
        operations: vec![row.operation],
        values: values.to_vec(),
        obligations: obligations.to_vec(),
        ..Default::default()
    }
}
pub(crate) fn memory_pointer(
    builder: &mut Builder<'_>,
    row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u32,
) -> Result<(VirtualRegisterId, u32), SelectedInstructionError> {
    displaced_memory_pointer(
        builder,
        place,
        pointer,
        byte_offset,
        byte_size,
        provenance(row),
    )
}

pub(crate) fn displaced_memory_pointer(
    builder: &mut Builder<'_>,
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
    byte_size: u32,
    provenance: SelectedInstructionProvenance,
) -> Result<(VirtualRegisterId, u32), SelectedInstructionError> {
    if builder
        .environment
        .admits_pointer_displacement(byte_offset, byte_size)
    {
        return Ok((pointer, byte_offset));
    }
    let address = transport_register(builder, place, byte_offset)?;
    builder.emit_address_offset(byte_offset, pointer, address, provenance)?;
    Ok((address, 0))
}

pub(crate) fn memory(
    builder: &mut Builder<'_>,
    row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    memory_at_origin(
        builder,
        invocation_memory_origin(row),
        place,
        byte_offset,
        byte_count,
        role,
    )
}

pub(crate) fn memory_at_origin(
    builder: &mut Builder<'_>,
    origin: selected_instructions::SelectedMemoryAccessOrigin,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    builder.transport.memory.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| invalid())?,
        ),
        origin,
        place,
        byte_offset,
        byte_count,
        role,
    });
    Ok(())
}

pub(crate) fn transport_at_byte_offset(
    builder: &mut Builder<'_>,
    input: VirtualRegisterId,
    output: VirtualRegisterId,
    byte_offset: u32,
    provenance: SelectedInstructionProvenance,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    builder.emit(
        if byte_offset == 0 {
            SelectedInstructionKind::CopyI64
        } else {
            SelectedInstructionKind::AddressOffset { byte_offset }
        },
        if byte_offset == 0 {
            builder.constraints.keys.copy_i64
        } else {
            builder
                .constraints
                .keys
                .address_offset
                .ok_or_else(|| invalid())?
        },
        &[input, output],
        provenance,
    )?;
    Ok(output)
}

pub(crate) fn storage_address(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    primitive_locals::address(source, builder, row)
}
