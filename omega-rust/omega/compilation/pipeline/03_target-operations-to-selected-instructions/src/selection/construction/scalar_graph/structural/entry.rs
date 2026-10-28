//! Capture incoming parameter ABI storage before ordinary graph operations.
use super::{
    Builder, IndirectPointerLocation, IntegerSign, IntegerType, LegalizedScalarFunction,
    ScalarType, SelectedInstructionProvenance, StructuralAccess, ValueLocation, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::transport_register;

pub(in crate::selection) fn entry(
    function: usize,
    source: &LegalizedScalarFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let Some(signature) = &source.structural else {
        return Ok(());
    };
    // Legalization owns place/producer custody. Entry captures only incoming
    // ABI storage; an empty parameter list follows the same call-plan check.
    let parameters = crate::structural_inputs::structural_unit_input::parameter_views(signature);
    if !crate::structural_inputs::structural_unit_input::accepts_graph(
        &source.call_plan,
        &parameters,
        &signature.structural_types,
    ) {
        return Err(SelectedInstructionError::unsupported_shape(function));
    }
    for (parameter_index, parameter) in signature.parameters.iter().enumerate() {
        let place = parameter.semantic.place;
        let owned_pointer = crate::structural_inputs::structural_unit_input::owned_indirect_pointer(
            &parameter.semantic,
            &parameter.target.placement,
        );
        if parameter.semantic.access == StructuralAccess::Owned
            && owned_pointer.is_none()
            && !crate::selection::aggregate_result_input::inline_argument_fragments(
                &parameter.target.placement,
            )
        {
            return Err(invalid());
        }
        if parameter.semantic.access == StructuralAccess::Owned
            && crate::selection::aggregate_result_input::inline_argument_fragments(
                &parameter.target.placement,
            )
        {
            // Inline stack bytes are an owned ABI payload, not a borrowed
            // referent. Their ABI coordinates need not meet the semantic
            // value's alignment. Retain the source pointer, then materialize
            // the same private home used by register/indirect arrivals when
            // the body needs an addressable value.
            // The frame encoder alone adds prologue/return bias.
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
                let input =
                    VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
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
                let offset = *value_byte_offset;
                let output = transport_register(builder, place, offset)?;
                builder.emit_copy_i64(input, output, SelectedInstructionProvenance::default())?;
                builder.transport.fragments.push((place, offset, output));
            }
            retain_owned_home(source, parameter, builder)?;
            continue;
        }
        let used = crate::selection::parameter_use::referent_used(source, place);
        // Owned arrivals can transfer into an aggregate without an intervening
        // read. Preserve their input storage independently of the body shape,
        // as for inline value fragments above.
        if owned_pointer.is_none() && !used {
            continue;
        }
        // An indirect owned argument points at the ABI's value copy, while a
        // borrowed argument points at its original referent. Both preserve
        // pointer bits across calls, without changing their semantic access.
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
        let output = transport_register(builder, place, 0)?;
        builder.emit_copy_i64(input, output, SelectedInstructionProvenance::default())?;
        builder.transport.pointers.push((place, output));
        if parameter.semantic.access == StructuralAccess::Owned {
            retain_owned_home(source, parameter, builder)?;
        }
    }
    Ok(())
}

/// Field loads need an addressable value, not an incoming register interpreted
/// as a pointer. Capture one function-owned home from the already retained ABI
/// fragments or ABI backing before any authored calls; unobserved values keep
/// their retained transport. This is value storage, never a borrowed referent.
fn retain_owned_home(
    source: &LegalizedScalarFunction,
    parameter: &legalized_operations::LegalizedCallUnitParameter,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let place = parameter.semantic.place;
    if !crate::selection::aggregate_result_input::parameter_home_required(source, place) {
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
    super::super::structural_case::memory(
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
        let mut offset = 0u32;
        while offset < shape.byte_size {
            let width = (shape.byte_size - offset).min(8);
            let value = transport_register(builder, place, offset)?;
            let (incoming, displacement) = super::displaced_memory_pointer(
                builder,
                place,
                incoming,
                offset,
                width,
                Default::default(),
            )?;
            super::super::structural_case::memory(
                builder,
                source.entry_block,
                place,
                offset,
                width,
                selected_instructions::SelectedMemoryAccessRole::ReadPlace,
            )?;
            super::super::aggregate_memory::load(builder, incoming, value, displacement, width)?;
            let (destination, displacement) = super::displaced_memory_pointer(
                builder,
                place,
                pointer,
                offset,
                width,
                Default::default(),
            )?;
            super::super::structural_case::memory(
                builder,
                source.entry_block,
                place,
                offset,
                width,
                selected_instructions::SelectedMemoryAccessRole::WritePlace,
            )?;
            super::super::aggregate_memory::store(
                builder,
                destination,
                value,
                displacement,
                width,
            )?;
            offset = offset.checked_add(width).ok_or_else(invalid)?;
        }
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
        let (pointer, displacement) = super::displaced_memory_pointer(
            builder,
            place,
            pointer,
            offset,
            width,
            Default::default(),
        )?;
        super::super::structural_case::memory(
            builder,
            source.entry_block,
            place,
            offset,
            width,
            selected_instructions::SelectedMemoryAccessRole::WritePlace,
        )?;
        super::super::aggregate_memory::store(builder, pointer, value, displacement, width)?;
    }
    builder.transport.pointers.push((place, pointer));
    Ok(())
}
