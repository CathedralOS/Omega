//! Independently replay incoming parameter placements and durable entry copies.
use super::*;

pub(in crate::selection) fn entry(
    source: &LegalizedScalarFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
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
            Err(replay.invalid())
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
    if !crate::structural_unit_input::accepts_graph(
        &source.call_plan,
        &parameters,
        &signature.structural_types,
    ) {
        return Err(replay.invalid());
    }
    for (parameter_index, parameter) in signature.parameters.iter().enumerate() {
        let place = parameter.semantic.place;
        let owned_pointer = crate::structural_unit_input::owned_indirect_pointer(
            &parameter.semantic,
            &parameter.target.placement,
        );
        if parameter.semantic.access == StructuralAccess::Owned
            && owned_pointer.is_none()
            && !crate::selection::aggregate_result_input::inline_argument_fragments(
                &parameter.target.placement,
            )
        {
            return Err(replay.invalid());
        }
        if parameter.semantic.access == StructuralAccess::Owned
            && crate::selection::aggregate_result_input::inline_argument_fragments(
                &parameter.target.placement,
            )
        {
            // Inline stack bytes are the owned payload, not a pointer to a
            // caller-owned referent. The incoming value copy remains valid
            // throughout this activation, including across calls. Retain its
            // address instead of making every payload fragment live at once.
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
                    .ok_or_else(|| replay.invalid())?;
                if source.call_plan.parameters.get(native_parameter)
                    != Some(&parameter.target.placement)
                {
                    return Err(replay.invalid());
                }
                let address = result(replay, place, 0)?;
                replay.check_instruction(
                    SelectedInstructionKind::FrameAddress {
                        slot: selected_instructions::FrameStorageSlotId::Incoming {
                            parameter_index: native_parameter
                                .try_into()
                                .map_err(|_| replay.invalid())?,
                            abi_stack_byte_offset: *base,
                        },
                        byte_offset: 0,
                    },
                    replay
                        .constraints
                        .keys
                        .frame_address
                        .ok_or_else(|| replay.invalid())?,
                    &[address],
                    &Default::default(),
                )?;
                for location in &parameter.target.placement.locations {
                    let ValueLocation::Stack {
                        stack_byte_offset,
                        value_byte_offset,
                        ..
                    } = location
                    else {
                        return Err(replay.invalid());
                    };
                    let offset = u32::from(*value_byte_offset);
                    if stack_byte_offset.checked_sub(*base) != Some(offset) {
                        return Err(replay.invalid());
                    }
                }
                replay.transport.pointers.push((place, address));
                continue;
            }
            for location in &parameter.target.placement.locations {
                let ValueLocation::Register {
                    register: machine_register,
                    value_byte_offset,
                    ..
                } = location
                else {
                    return Err(replay.invalid());
                };
                let fixed = environment
                    .fixed_register_view(*machine_register)
                    .ok_or_else(|| replay.invalid())?;
                let input = register(
                    replay,
                    VirtualRegisterOrigin::StructuralParameter {
                        place,
                        parameter_index,
                    },
                    Some(fixed),
                )?;
                let offset = u32::from(*value_byte_offset);
                let output = result(replay, place, offset)?;
                replay.check_instruction(
                    SelectedInstructionKind::CopyI64,
                    replay.constraints.keys.copy_i64,
                    &[input, output],
                    &SelectedInstructionProvenance::default(),
                )?;
                replay.transport.fragments.push((place, offset, output));
            }
            retain_owned_home(source, parameter, replay)?;
            continue;
        }
        if owned_pointer.is_none() && !crate::selection::established_view_input::transferred(source, place) && !source.blocks.iter().flat_map(|block|&block.instructions).any(|row| match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldRead { source: argument, .. } => argument.place == place,
            LegalizedScalarInstructionKind::StructuralScalarFieldStore { destination, .. }
            | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { destination, .. } => destination.place == place,
            LegalizedScalarInstructionKind::PrimitiveScalarRead { source, .. }
            | LegalizedScalarInstructionKind::StructuralCaseMembership { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. } => *source == place,
            LegalizedScalarInstructionKind::Call(call)=>call.arguments.iter().any(|argument|matches!(argument,LegalizedScalarArgument::Structural {semantic,..} if semantic.place==place)),_=>false,
        }) {continue;}
        // Replay the exact incoming pointer location, not the caller's copy
        // offset or a same-sized inline value. Access remains independently
        // reconstructed from the structural declaration and full call plan.
        let stack_pointer_offset = match owned_pointer {
            Some(IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            }) => Some(stack_byte_offset),
            _ => {
                crate::structural_reference_input::stack_pointer_offset(&parameter.target.placement)
            }
        };
        if let Some(abi_stack_byte_offset) = stack_pointer_offset {
            let native_parameter = source
                .parameters
                .len()
                .checked_add(parameter_index)
                .ok_or_else(|| replay.invalid())?;
            let address = result(replay, place, 0)?;
            replay.check_instruction(
                SelectedInstructionKind::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::Incoming {
                        parameter_index: native_parameter
                            .try_into()
                            .map_err(|_| replay.invalid())?,
                        abi_stack_byte_offset,
                    },
                    byte_offset: 0,
                },
                replay
                    .constraints
                    .keys
                    .frame_address
                    .ok_or_else(|| replay.invalid())?,
                &[address],
                &SelectedInstructionProvenance::default(),
            )?;
            let pointer = result(replay, place, 0)?;
            replay.check_instruction(
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                replay
                    .constraints
                    .keys
                    .load64
                    .ok_or_else(|| replay.invalid())?,
                &[address, pointer],
                &SelectedInstructionProvenance::default(),
            )?;
            replay.transport.pointers.push((place, pointer));
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
            _ => return Err(replay.invalid()),
        };
        let fixed = environment
            .fixed_register_view(*pointer)
            .ok_or_else(|| replay.invalid())?;
        let input = register(
            replay,
            VirtualRegisterOrigin::StructuralParameter {
                place,
                parameter_index,
            },
            Some(fixed),
        )?;
        let output = result(replay, place, 0)?;
        replay.check_instruction(
            SelectedInstructionKind::CopyI64,
            replay.constraints.keys.copy_i64,
            &[input, output],
            &SelectedInstructionProvenance::default(),
        )?;
        replay.transport.pointers.push((place, output));
    }
    Ok(())
}

/// Reconstruct the input home's extent and every write from the declared value
/// placement. A producer-provided address or partial fragment roster cannot
/// stand in for the owned input, even when a later field load has a valid width.
fn retain_owned_home(
    source: &LegalizedScalarFunction,
    parameter: &legalized_operations::LegalizedCallUnitParameter,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let place = parameter.semantic.place;
    if !crate::selection::record_input::parameter_home_required(source, place) {
        return Ok(());
    }
    let slot = selected_instructions::LocalStorageSlotId::StructuralParameter { place };
    let shape = parameter.target.shape;
    replay
        .transport
        .local_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: u32::from(shape.byte_size),
            alignment: shape.alignment,
        });
    let pointer = result(replay, place, 0)?;
    super::super::structural_case::memory(
        replay,
        source.entry_block,
        place,
        0,
        u32::from(shape.byte_size),
        selected_instructions::SelectedMemoryAccessRole::AddressLocal { slot },
    )?;
    replay.check_instruction(
        SelectedInstructionKind::FrameAddress {
            slot: selected_instructions::FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        },
        replay
            .constraints
            .keys
            .frame_address
            .ok_or_else(|| replay.invalid())?,
        &[pointer],
        &Default::default(),
    )?;
    for location in &parameter.target.placement.locations {
        let (offset, width) = match location {
            ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            }
            | ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                ..
            } => (u32::from(*value_byte_offset), *byte_size),
            _ => return Err(replay.invalid()),
        };
        let value = replay
            .transport
            .fragments
            .iter()
            .find(|(stored, byte_offset, _)| *stored == place && *byte_offset == offset)
            .map(|(_, _, value)| *value)
            .ok_or_else(|| replay.invalid())?;
        super::super::structural_case::memory(
            replay,
            source.entry_block,
            place,
            offset,
            u32::from(width),
            selected_instructions::SelectedMemoryAccessRole::WritePlace,
        )?;
        super::super::aggregate_memory::store(replay, pointer, value, offset, width)?;
    }
    replay.transport.pointers.push((place, pointer));
    Ok(())
}
