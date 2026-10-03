//! Transport owned aggregate bytes through the independently selected call ABI.
//! Indirection copies the current value into call-owned backing; forwarding the
//! input pointer would alias a copyable value with the callee's private storage.
use super::{
    LegalizedScalarFunction, SelectedInstructionId, SelectedInstructionKind, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use calling_conventions::ValueLocation;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedMemoryAccessRole};

pub(super) fn argument(
    source: &LegalizedScalarFunction,
    operation: &legalized_operations::LegalizedScalarInstruction,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    replay: &mut Replay<'_>,
) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
    let place = semantic.place;
    let root_shape =
        crate::selection::scalar_call_abi::owned_value_shape(source, target.root_structural_type)
            .ok_or_else(|| SelectedInstructionError::custody())?;
    let slot = match target.source {
        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            Some(LocalStorageSlotId::Structural {
                operation: psi_operation,
                place,
            })
        }
        target_operations::TargetStructuralArgumentSource::BlockParameter { block, place } => {
            Some(LocalStorageSlotId::StructuralBlockParameter { block, place })
        }
        target_operations::TargetStructuralArgumentSource::Placement(_) => None,
        _ => return Err(SelectedInstructionError::custody()),
    };
    // Exact semantic call replay retains the empty value's producer and type;
    // zero ABI fragments cannot justify a fabricated physical home.
    if crate::selection::scalar_call_abi::empty_aggregate_placement(&target.destination)
        && target.shape == target.destination.shape
    {
        return Ok(Vec::new());
    }
    if target.shape.byte_size != target.destination.shape.byte_size
        || target.shape.alignment != target.destination.shape.alignment
        || !crate::selection::aggregate_result_input::owned_argument_placement(&target.destination)
    {
        return Err(SelectedInstructionError::custody());
    }
    let block = source
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|row| row.operation == operation.operation)
        })
        .ok_or_else(|| SelectedInstructionError::custody())?
        .id;
    let pointer = if let Some(slot) = slot {
        if replay
            .transport
            .local_slots
            .iter()
            .filter(|home| {
                home.id == slot
                    && home.byte_size == root_shape.byte_size
                    && home.alignment == root_shape.alignment
            })
            .count()
            != 1
        {
            return Err(SelectedInstructionError::custody());
        }
        let pointer = super::structural_case::temporary(replay, place, 0, false)?;
        super::structural_case::memory(
            replay,
            block,
            place,
            0,
            root_shape.byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        replay.check_instruction(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            replay
                .constraints
                .keys
                .frame_address
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[pointer],
            &Default::default(),
        )?;
        Some(pointer)
    } else {
        replay
            .transport
            .pointers
            .iter()
            .find(|(owner, _)| *owner == place)
            .map(|(_, pointer)| *pointer)
    };
    let indirect = crate::selection::aggregate_result_input::indirect_argument(&target.destination);
    let outgoing_plan = match &operation.kind {
        legalized_operations::LegalizedScalarInstructionKind::Call(call) => &call.call_plan,
        legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(call) => {
            &call.binding.boundary_entry_plan.call
        }
        _ => return Err(SelectedInstructionError::custody()),
    };
    let stack_base = target
        .destination
        .locations
        .iter()
        .filter_map(|location| match location {
            ValueLocation::Stack {
                stack_byte_offset, ..
            } => Some(*stack_byte_offset),
            _ => None,
        })
        .min();
    let outgoing_offset = indirect.map(|(_, offset)| offset).or(stack_base);
    let outgoing = if let Some(stack_byte_offset) = outgoing_offset {
        let byte_size = if indirect.is_some() {
            target.shape.byte_size
        } else {
            target
                .destination
                .locations
                .iter()
                .filter_map(|location| match location {
                    ValueLocation::Stack {
                        stack_byte_offset,
                        byte_size,
                        ..
                    } => Some(stack_byte_offset.checked_add(*byte_size)),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
                .and_then(|ends| ends.into_iter().max())
                .and_then(|end| end.checked_sub(stack_byte_offset))
                .ok_or_else(|| SelectedInstructionError::custody())?
        };
        let slot = selected_instructions::OutgoingArgumentSlotId {
            operation: operation.operation,
            argument_index: argument_index
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
            role: if indirect.is_some() {
                selected_instructions::OutgoingArgumentSlotRole::ValueCopy
            } else {
                selected_instructions::OutgoingArgumentSlotRole::Argument
            },
        };
        if replay
            .transport
            .slots
            .iter()
            .any(|existing| existing.id == slot)
        {
            return Err(SelectedInstructionError::custody());
        }
        let alignment = target
            .destination
            .locations
            .iter()
            .filter_map(|location| {
                if let ValueLocation::Stack { alignment, .. } = location {
                    Some(*alignment)
                } else {
                    None
                }
            })
            .fold(target.shape.alignment, u16::max);
        let alignment = if indirect.is_some()
            && outgoing_plan.policy == calling_conventions::CallingPolicy::MicrosoftX64
        {
            alignment.max(16)
        } else {
            alignment
        };
        replay
            .transport
            .slots
            .push(selected_instructions::SelectedOutgoingArgumentSlot {
                id: slot,
                byte_size,
                alignment,
                abi_stack_byte_offset: stack_byte_offset,
            });
        let address = super::structural_case::temporary(replay, place, 0, false)?;
        outgoing_memory(
            replay,
            operation.operation,
            place,
            0,
            byte_size,
            SelectedMemoryAccessRole::AddressOutgoing { slot },
        )?;
        replay.check_instruction(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Outgoing(slot),
                byte_offset: 0,
            },
            replay
                .constraints
                .keys
                .frame_address
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[address],
            &Default::default(),
        )?;
        Some((slot, address))
    } else {
        None
    };
    let mut registers = Vec::new();
    let fragments = if indirect.is_some() {
        (0..target.shape.byte_size)
            .step_by(8)
            .map(|offset| {
                (
                    offset,
                    (target.shape.byte_size - offset).min(8),
                    None,
                    Some(offset),
                )
            })
            .collect::<Vec<_>>()
    } else {
        target
            .destination
            .locations
            .iter()
            .map(|location| match location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => Ok(vec![(
                    *value_byte_offset,
                    *byte_size,
                    Some(*register),
                    None,
                )]),
                ValueLocation::Stack {
                    value_byte_offset,
                    byte_size,
                    stack_byte_offset,
                    ..
                } => {
                    let stack_offset = stack_byte_offset
                        .checked_sub(stack_base.ok_or_else(|| SelectedInstructionError::custody())?)
                        .ok_or_else(|| SelectedInstructionError::custody())?;
                    (0..*byte_size)
                        .step_by(8)
                        .map(|offset| {
                            Ok((
                                value_byte_offset
                                    .checked_add(offset)
                                    .ok_or_else(|| SelectedInstructionError::custody())?,
                                (*byte_size - offset).min(8),
                                None,
                                Some(
                                    stack_offset
                                        .checked_add(offset)
                                        .ok_or_else(|| SelectedInstructionError::custody())?,
                                ),
                            ))
                        })
                        .collect::<Result<Vec<_>, SelectedInstructionError>>()
                }
                _ => Err(SelectedInstructionError::custody()),
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    };
    for (value_byte_offset, byte_size, register, stack_offset) in fragments {
        let offset = value_byte_offset;
        // Read the selected subtree at its root-relative offset, while the
        // callee's ABI fragments and outgoing copy remain zero-based.
        let source_offset = target
            .source_byte_offset
            .checked_add(offset)
            .ok_or_else(|| SelectedInstructionError::custody())?;
        let output = super::structural_case::temporary(replay, place, offset, false)?;
        if let Some(pointer) = pointer {
            super::structural_case::memory(
                replay,
                block,
                place,
                source_offset,
                byte_size,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            super::aggregate_memory::load(replay, pointer, output, source_offset, byte_size)?;
        } else {
            let input = replay
                .transport
                .fragments
                .iter()
                .find(|(owner, position, _)| *owner == place && *position == source_offset)
                .map(|(_, _, register)| *register)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            replay.check_instruction(
                SelectedInstructionKind::CopyI64,
                replay.constraints.keys.copy_i64,
                &[input, output],
                &Default::default(),
            )?;
        }
        if let Some(stack_offset) = stack_offset {
            let (slot, address) = outgoing.ok_or_else(|| SelectedInstructionError::custody())?;
            outgoing_memory(
                replay,
                operation.operation,
                place,
                stack_offset,
                byte_size,
                SelectedMemoryAccessRole::WriteOutgoing { slot },
            )?;
            super::aggregate_memory::store(replay, address, output, stack_offset, byte_size)?;
        } else {
            let register = register.ok_or_else(|| SelectedInstructionError::custody())?;
            registers.push(argument_register(
                replay, place, offset, byte_size, register, output,
            )?);
        }
    }
    if let Some((pointer_location, _)) = indirect {
        let (_, address) = outgoing.ok_or_else(|| SelectedInstructionError::custody())?;
        match pointer_location {
            calling_conventions::IndirectPointerLocation::Register(_) => registers.push(address),
            calling_conventions::IndirectPointerLocation::Stack {
                stack_byte_offset,
                alignment,
            } => {
                let slot = selected_instructions::OutgoingArgumentSlotId {
                    operation: operation.operation,
                    argument_index: argument_index
                        .try_into()
                        .map_err(|_| SelectedInstructionError::custody())?,
                    role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                };
                if replay
                    .transport
                    .slots
                    .iter()
                    .any(|existing| existing.id == slot)
                {
                    return Err(SelectedInstructionError::custody());
                }
                replay
                    .transport
                    .slots
                    .push(selected_instructions::SelectedOutgoingArgumentSlot {
                        id: slot,
                        byte_size: 8,
                        alignment,
                        abi_stack_byte_offset: stack_byte_offset,
                    });
                outgoing_memory(
                    replay,
                    operation.operation,
                    place,
                    0,
                    8,
                    SelectedMemoryAccessRole::WriteOutgoing { slot },
                )?;
                replay.check_instruction(
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Outgoing(slot),
                        byte_offset: 0,
                    },
                    replay
                        .constraints
                        .keys
                        .store64
                        .ok_or_else(|| SelectedInstructionError::custody())?,
                    &[address],
                    &Default::default(),
                )?;
            }
        }
    }
    Ok(registers)
}

/// Reconstruct the ABI-bank transfer from the fragment's retained destination.
fn argument_register(
    replay: &mut Replay<'_>,
    place: semantic_vocabulary::PlaceId,
    byte_offset: u32,
    byte_size: u32,
    register: calling_conventions::MachineRegister,
    input: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if !matches!(
        register,
        calling_conventions::MachineRegister::Aarch64V(_)
            | calling_conventions::MachineRegister::X86Xmm(_)
    ) {
        return Ok(input);
    }
    let format = match byte_size {
        4 => semantic_vocabulary::IeeeFloatFormat::Binary32,
        8 => semantic_vocabulary::IeeeFloatFormat::Binary64,
        _ => return Err(replay.invalid()),
    };
    let (kind, key) = crate::selection::scalar_call_abi::outgoing_float_transfer(
        semantic_vocabulary::ScalarType::IeeeFloat(format),
        &replay.constraints.keys,
    )
    .ok_or_else(|| replay.invalid())?;
    let class = replay
        .environment
        .constraint(key)
        .and_then(|row| row.operands.get(1))
        .ok_or_else(|| replay.invalid())?
        .class;
    let origin = selected_instructions::VirtualRegisterOrigin::AbiTransport {
        instruction: SelectedInstructionId(
            replay
                .instruction_cursor
                .try_into()
                .map_err(|_| replay.invalid())?,
        ),
        place,
        byte_offset,
    };
    let scalar_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .map_err(|_| replay.invalid())?,
    );
    let retained = replay
        .selected
        .virtual_registers
        .get(replay.register_cursor)
        .ok_or_else(|| replay.invalid())?;
    if retained.id.0 as usize != replay.register_cursor
        || retained.class != class
        || retained.origin != origin
        || retained.scalar_type != scalar_type
        || retained.definition_site.is_some()
        || retained.entry_fixed_view.is_some()
    {
        return Err(replay.invalid());
    }
    let output = retained.id;
    replay.register_cursor += 1;
    replay.check_instruction(kind, key, &[input, output], &Default::default())?;
    Ok(output)
}

/// Outgoing ABI storage belongs to this call occurrence, not to a new source place.
fn outgoing_memory(
    replay: &mut Replay<'_>,
    operation: semantic_vocabulary::OperationId,
    place: semantic_vocabulary::PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    replay
        .transport
        .memory
        .push(selected_instructions::SelectedMemoryAccess {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| SelectedInstructionError::custody())?,
            ),
            origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(operation),
            place,
            byte_offset,
            byte_count,
            role,
        });
    Ok(())
}
