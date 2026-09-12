//! Capture incoming parameter ABI storage before ordinary graph operations.
use super::*;

pub(in crate::selection) fn entry(
    function: usize,
    source: &LegalizedScalarFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    if crate::unobserved_owned_input::accepts(source) {
        return Ok(());
    }
    let Some(signature) = &source.structural else {
        return Ok(());
    };
    if signature.parameters.is_empty() {
        return if crate::selection::primitive_local_input::accepts(source)
            || crate::selection::literal_storage_input::accepts(source)
            || crate::selection::read_result_input::accepts(source)
            || crate::selection::aggregate_result_input::has_local_aggregates(source)
        {
            Ok(())
        } else {
            Err(SelectedInstructionError::UnsupportedSourceShape { function })
        };
    }
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| crate::structural_unit_input::Parameter {
            semantic: &parameter.semantic,
            target: &parameter.target,
        })
        .collect::<Vec<_>>();
    let legacy_indirect = crate::structural_unit_input::accepts(
        &source.call_plan,
        &parameters,
        &signature.structural_types,
    );
    if !legacy_indirect
        && !crate::structural_unit_input::accepts_graph(
            &source.call_plan,
            &parameters,
            &signature.structural_types,
        )
    {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    for (parameter_index, parameter) in signature.parameters.iter().enumerate() {
        let place = parameter.semantic.place;
        if parameter.semantic.access == StructuralAccess::Owned
            && !legacy_indirect
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
            // Inline stack bytes are the owned payload, not a pointer to a
            // caller-owned referent. Capture every exact fragment at entry so
            // later calls use ordinary value preservation, just as register
            // parameters do. The frame encoder alone adds prologue/return bias.
            if let Some(ValueLocation::Stack {
                stack_byte_offset: base,
                ..
            }) = parameter.target.placement.locations.first()
            {
                let native_parameter = source
                    .parameters
                    .len()
                    .checked_add(parameter_index)
                    .ok_or_else(invalid)?;
                if source.call_plan.parameters.get(native_parameter)
                    != Some(&parameter.target.placement)
                {
                    return Err(invalid());
                }
                let address = transport_register(builder, place, 0)?;
                builder.emit(
                    SelectedInstructionKind::FrameAddress {
                        slot: selected_instructions::FrameStorageSlotId::Incoming {
                            parameter_index: native_parameter.try_into().map_err(|_| invalid())?,
                            abi_stack_byte_offset: *base,
                        },
                        byte_offset: 0,
                    },
                    builder.constraints.keys.frame_address.ok_or_else(invalid)?,
                    &[address],
                    Default::default(),
                )?;
                for location in &parameter.target.placement.locations {
                    let ValueLocation::Stack {
                        stack_byte_offset,
                        value_byte_offset,
                        byte_size,
                        ..
                    } = location
                    else {
                        return Err(invalid());
                    };
                    let offset = u32::from(*value_byte_offset);
                    if stack_byte_offset.checked_sub(*base) != Some(offset) {
                        return Err(invalid());
                    }
                    let output = transport_register(builder, place, offset)?;
                    super::super::structural_case::memory(
                        builder,
                        source.entry_block,
                        place,
                        offset,
                        u32::from(*byte_size),
                        selected_instructions::SelectedMemoryAccessRole::ReadPlace,
                    )?;
                    super::super::aggregate_memory::load(
                        builder, address, output, offset, *byte_size,
                    )?;
                    builder.transport.fragments.push((place, offset, output));
                }
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
                    .ok_or_else(invalid)?;
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
                let offset = u32::from(*value_byte_offset);
                let output = transport_register(builder, place, offset)?;
                builder.emit(
                    SelectedInstructionKind::CopyI64,
                    builder.constraints.keys.copy_i64,
                    &[input, output],
                    SelectedInstructionProvenance::default(),
                )?;
                builder.transport.fragments.push((place, offset, output));
            }
            continue;
        }
        let used = crate::selection::established_view_input::transferred(source, place) || source.blocks.iter().flat_map(|block| &block.instructions).any(|row| match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldStore { destination, .. }
            | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { destination, .. } => destination.place == place,
            LegalizedScalarInstructionKind::PrimitiveScalarRead { source, .. }
            | LegalizedScalarInstructionKind::StructuralCaseMembership { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. } => *source == place,
            LegalizedScalarInstructionKind::Call(call) => call.arguments.iter().any(|argument| matches!(argument,LegalizedScalarArgument::Structural {semantic,..} if semantic.place == place)),
            _ => false,
        });
        if !used {
            continue;
        }
        if let Some(abi_stack_byte_offset) =
            crate::structural_reference_input::stack_pointer_offset(&parameter.target.placement)
        {
            let native_parameter = source
                .parameters
                .len()
                .checked_add(parameter_index)
                .ok_or_else(invalid)?;
            let address = transport_register(builder, place, 0)?;
            builder.emit(
                SelectedInstructionKind::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::Incoming {
                        parameter_index: native_parameter.try_into().map_err(|_| invalid())?,
                        abi_stack_byte_offset,
                    },
                    byte_offset: 0,
                },
                builder.constraints.keys.frame_address.ok_or_else(invalid)?,
                &[address],
                SelectedInstructionProvenance::default(),
            )?;
            let pointer = transport_register(builder, place, 0)?;
            builder.emit(
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                builder.constraints.keys.load64.ok_or_else(invalid)?,
                &[address, pointer],
                SelectedInstructionProvenance::default(),
            )?;
            builder.transport.pointers.push((place, pointer));
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
            .ok_or_else(invalid)?;
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
        builder.emit(
            SelectedInstructionKind::CopyI64,
            builder.constraints.keys.copy_i64,
            &[input, output],
            SelectedInstructionProvenance::default(),
        )?;
        builder.transport.pointers.push((place, output));
    }
    Ok(())
}
