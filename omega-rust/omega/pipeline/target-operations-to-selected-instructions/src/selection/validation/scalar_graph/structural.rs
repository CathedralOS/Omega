//! Independent replay of place-backed snapshots, copy accesses and call operands.
use super::*;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use selected_instructions::{
    OutgoingArgumentSlotId, SelectedBoundarySettlement, SelectedCallContract, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedOutgoingArgumentSlot,
};
use semantic_vocabulary::{IntegerType, PlaceId};

#[derive(Default)]
pub(super) struct Transport {
    pointers: Vec<(PlaceId, VirtualRegisterId)>,
    pub slots: Vec<SelectedOutgoingArgumentSlot>,
    pub calls: Vec<SelectedCallContract>,
    pub memory: Vec<SelectedMemoryAccess>,
    pub settlements: Vec<SelectedBoundarySettlement>,
}

fn register(
    replay: &mut Replay<'_>,
    origin: VirtualRegisterOrigin,
    fixed: Option<RegisterViewId>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let proposed = replay
        .selected
        .virtual_registers
        .get(replay.register_cursor)
        .ok_or_else(|| replay.invalid())?;
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?;
    if proposed.id.0 as usize != replay.register_cursor
        || proposed.origin != origin
        || proposed.definition_site.is_some()
        || proposed.entry_fixed_view != fixed
        || proposed.class != replay.class
        || proposed.scalar_type != ScalarType::Integer(integer)
    {
        return Err(replay.invalid());
    }
    replay.register_cursor += 1;
    Ok(proposed.id)
}
fn result(
    replay: &mut Replay<'_>,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    register(
        replay,
        VirtualRegisterOrigin::AbiTransport {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| replay.invalid())?,
            ),
            place,
            byte_offset,
        },
        None,
    )
}

pub(super) fn entry(
    source: &LegalizedScalarFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let Some(signature) = &source.structural else {
        return Ok(());
    };
    // Ranked structural state is retained ownership custody, not a physical read.
    // Its exact signature and cleanup are independently checked at legalization.
    if source.ranked.is_some()
        && source.blocks.iter().all(|block| {
            block
                .instructions
                .iter()
                .all(|row| !matches!(row.kind, LegalizedScalarInstructionKind::Call(_)))
        })
    {
        return Ok(());
    }
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| crate::structural_unit_input::Parameter {
            semantic: &parameter.semantic,
            target: &parameter.target,
        })
        .collect::<Vec<_>>();
    if !crate::structural_unit_input::accepts(
        &source.call_plan,
        &parameters,
        &signature.structural_types,
    ) && !crate::structural_unit_input::accepts_borrowed_view(
        &source.call_plan,
        &parameters,
        &signature.structural_types,
    ) {
        return Err(replay.invalid());
    }
    for (parameter_index, parameter) in signature.parameters.iter().enumerate() {
        let place = parameter.semantic.place;
        if !source.blocks.iter().flat_map(|block|&block.instructions).any(|row| match &row.kind {
            LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. } => *source == place,
            LegalizedScalarInstructionKind::Call(call)=>call.arguments.iter().any(|argument|matches!(argument,LegalizedScalarArgument::Structural {semantic,..} if semantic.place==place)),_=>false,
        }) {continue;}
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
            ] if parameter.semantic.access == StructuralAccess::SharedBorrow => register,
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

pub(super) fn operation(
    source: &LegalizedScalarFunction,
    node: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
) -> Result<bool, SelectedInstructionError> {
    if let LegalizedScalarInstructionKind::BoundarySettlement(settlement) = &node.kind {
        if node.result.is_some()
            || node.operation != settlement.operation
            || node.fuel != settlement.fuel
            || node.effect != settlement.effect
            || node.ownership != settlement.ownership
        {
            return Err(replay.invalid());
        }
        replay
            .transport
            .settlements
            .push(SelectedBoundarySettlement {
                block: replay.block.id,
                instruction_index: replay
                    .block_cursor
                    .try_into()
                    .map_err(|_| replay.invalid())?,
                settlement: settlement.clone(),
            });
        return Ok(true);
    }
    if node.result.is_some() {
        return Ok(false);
    }
    let LegalizedScalarInstructionKind::Call(call) = &node.kind else {
        return Err(replay.invalid());
    };
    let signature = source.structural.as_ref().ok_or_else(|| replay.invalid())?;
    call.validate_shape().map_err(|_| replay.invalid())?;
    call.validate_source(&node.ownership)
        .map_err(|_| replay.invalid())?;
    if call.result_placement.is_some()
        || call.call_plan != source.call_plan
        || call.arguments.len() != signature.parameters.len()
    {
        return Err(replay.invalid());
    }
    let mut pointers = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        let LegalizedScalarArgument::Structural { semantic, target } = argument else {
            return Err(replay.invalid());
        };
        let parameter = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == semantic.place)
            .ok_or_else(|| replay.invalid())?;
        if semantic.access != StructuralAccess::Owned
            || !semantic.path.is_empty()
            || target.place != semantic.place
            || target.source != parameter.target.placement
            || target.destination != call.call_plan.parameters[argument_index]
            || target.source_byte_offset != 0
        {
            return Err(replay.invalid());
        }
        let [
            ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(pointer),
                copy_stack_byte_offset: Some(offset),
                byte_size,
                alignment,
            },
        ] = target.destination.locations.as_slice()
        else {
            return Err(replay.invalid());
        };
        let slot = OutgoingArgumentSlotId {
            operation: node.operation,
            argument_index: argument_index.try_into().map_err(|_| replay.invalid())?,
        };
        replay.transport.slots.push(SelectedOutgoingArgumentSlot {
            id: slot,
            byte_size: u32::from(*byte_size),
            alignment: *alignment,
            abi_stack_byte_offset: *offset,
        });
        let input = replay
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == semantic.place)
            .map(|(_, register)| *register)
            .ok_or_else(|| replay.invalid())?;
        for byte_offset in [0, 8] {
            let value = result(replay, semantic.place, byte_offset)?;
            memory(
                replay,
                node,
                semantic.place,
                byte_offset,
                8,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            replay.check_instruction(
                SelectedInstructionKind::Load64 { byte_offset },
                replay
                    .constraints
                    .keys
                    .load64
                    .ok_or_else(|| replay.invalid())?,
                &[input, value],
                &provenance(node),
            )?;
            memory(
                replay,
                node,
                semantic.place,
                byte_offset,
                8,
                SelectedMemoryAccessRole::WriteOutgoing { slot },
            )?;
            replay.check_instruction(
                SelectedInstructionKind::Store64 { slot, byte_offset },
                replay
                    .constraints
                    .keys
                    .store64
                    .ok_or_else(|| replay.invalid())?,
                &[value],
                &provenance(node),
            )?;
        }
        let address = result(replay, semantic.place, 0)?;
        memory(
            replay,
            node,
            semantic.place,
            0,
            u32::from(*byte_size),
            SelectedMemoryAccessRole::AddressOutgoing { slot },
        )?;
        replay.check_instruction(
            SelectedInstructionKind::FrameAddress {
                slot,
                byte_offset: 0,
            },
            replay
                .constraints
                .keys
                .frame_address
                .ok_or_else(|| replay.invalid())?,
            &[address],
            &provenance(node),
        )?;
        pointers.push((
            address,
            environment
                .fixed_register_view(*pointer)
                .ok_or_else(|| replay.invalid())?,
        ));
    }
    let key = replay
        .constraints
        .keys
        .call_unit
        .ok_or_else(|| replay.invalid())?;
    let constraint = environment
        .constraint(key)
        .ok_or_else(|| replay.invalid())?;
    if constraint.operands.len() != pointers.len()
        || constraint
            .operands
            .iter()
            .zip(&pointers)
            .any(|(operand, (_, view))| {
                operand.access != RegisterOperandAccess::Use || operand.fixed_view != Some(*view)
            })
    {
        return Err(replay.invalid());
    }
    replay.transport.calls.push(SelectedCallContract {
        instruction: SelectedInstructionId(
            replay
                .instruction_cursor
                .try_into()
                .map_err(|_| replay.invalid())?,
        ),
        operation: node.operation,
        call: call.clone(),
        effect: node.effect,
        ownership: node.ownership.clone(),
    });
    let provenance = SelectedInstructionProvenance {
        operations: vec![node.operation],
        fuel: node.fuel.clone(),
        obligations: call.requirement_obligations.clone(),
        ..Default::default()
    };
    replay.check_instruction(
        SelectedInstructionKind::CallUnit {
            callee: call.callee,
        },
        key,
        &pointers
            .iter()
            .map(|(register, _)| *register)
            .collect::<Vec<_>>(),
        &provenance,
    )?;
    Ok(true)
}
pub(super) fn byte_observation(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    match row.kind {
        LegalizedScalarInstructionKind::ByteSequenceRead { .. } => byte_sequence_read(replay, row),
        LegalizedScalarInstructionKind::ByteSequenceLength {
            source,
            length_byte_offset,
        } => byte_sequence_length(replay, row, source, length_byte_offset),
        _ => Err(replay.invalid()),
    }
}

fn byte_sequence_read(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ByteSequenceRead {
        source,
        index,
        length,
        obligation,
        accepted_fact,
    } = row.kind
    else {
        return Err(replay.invalid());
    };
    let definition = row.result.ok_or_else(|| replay.invalid())?;
    let (_, index_register, _, index_type) =
        replay.resolve(index).ok_or_else(|| replay.invalid())?;
    if definition.scalar_type
        != ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| replay.invalid())?,
        )
        || index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
            )
    {
        return Err(replay.invalid());
    }
    let descriptor = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(|| replay.invalid())?;
    let pointer = result(replay, source, 0)?;
    memory(
        replay,
        row,
        source,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        replay
            .constraints
            .keys
            .load64
            .ok_or_else(|| replay.invalid())?,
        &[descriptor, pointer],
        &provenance(row),
    )?;
    let output = replay.result_register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    memory(
        replay,
        row,
        source,
        0,
        1,
        SelectedMemoryAccessRole::ReadByteSequence {
            index,
            length,
            obligation,
            accepted_fact,
        },
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load8Indexed,
        replay
            .constraints
            .keys
            .load8_indexed
            .ok_or_else(|| replay.invalid())?,
        &[pointer, index_register, output],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}

fn byte_sequence_length(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
    length_byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = row.result.ok_or_else(|| replay.invalid())?;
    if length_byte_offset != 8
        || result.scalar_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
            )
    {
        return Err(replay.invalid());
    }
    let input = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(|| replay.invalid())?;
    let output =
        replay.result_register(result.value, result.definition_site, result.scalar_type)?;
    memory(
        replay,
        row,
        source,
        length_byte_offset,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    let provenance = SelectedInstructionProvenance {
        operations: vec![row.operation],
        values: vec![result.value],
        fuel: row.fuel.clone(),
        ..Default::default()
    };
    replay.check_instruction(
        SelectedInstructionKind::Load64 {
            byte_offset: length_byte_offset,
        },
        replay
            .constraints
            .keys
            .load64
            .ok_or_else(|| replay.invalid())?,
        &[input, output],
        &provenance,
    )?;
    Ok(output)
}

fn provenance(row: &LegalizedScalarInstruction) -> SelectedInstructionProvenance {
    SelectedInstructionProvenance {
        operations: vec![row.operation],
        ..Default::default()
    }
}
fn memory(
    replay: &mut Replay<'_>,
    node: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    replay.transport.memory.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(
            replay
                .instruction_cursor
                .try_into()
                .map_err(|_| replay.invalid())?,
        ),
        operation: node.operation,
        place,
        byte_offset,
        byte_count,
        role,
    });
    Ok(())
}
