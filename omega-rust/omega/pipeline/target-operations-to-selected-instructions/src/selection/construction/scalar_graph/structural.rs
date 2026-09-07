//! Place-backed ABI transport in the ordinary instruction stream.
use super::*;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use selected_instructions::{
    OutgoingArgumentSlotId, SelectedBoundarySettlement, SelectedCallContract, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedOutgoingArgumentSlot,
};
use semantic_vocabulary::{IntegerType, PlaceId};

#[derive(Default)]
pub(super) struct Transport {
    pub pointers: Vec<(PlaceId, VirtualRegisterId)>,
    pub slots: Vec<SelectedOutgoingArgumentSlot>,
    pub calls: Vec<SelectedCallContract>,
    pub memory: Vec<SelectedMemoryAccess>,
    pub settlements: Vec<SelectedBoundarySettlement>,
}

fn invalid() -> SelectedInstructionError {
    SelectedInstructionError::SourceCustodyMismatch
}

fn transport_register(
    builder: &mut Builder<'_>,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
    let instruction = SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| invalid())?,
    );
    builder.registers.push(VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
        ),
        class: builder.class,
        origin: VirtualRegisterOrigin::AbiTransport {
            instruction,
            place,
            byte_offset,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    Ok(id)
}

pub(super) fn entry(
    function: usize,
    source: &LegalizedScalarFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
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
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    for (parameter_index, parameter) in signature.parameters.iter().enumerate() {
        let place = parameter.semantic.place;
        let used = source.blocks.iter().flat_map(|block| &block.instructions).any(|row| match &row.kind {
            LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. } => *source == place,
            LegalizedScalarInstructionKind::Call(call) => call.arguments.iter().any(|argument| matches!(argument,LegalizedScalarArgument::Structural {semantic,..} if semantic.place == place)),
            _ => false,
        });
        if !used {
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
            ] if parameter.semantic.access == StructuralAccess::SharedBorrow => register,
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

pub(super) fn operation(
    source: &LegalizedScalarFunction,
    block: SelectedBlockId,
    block_start: usize,
    row: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<bool, SelectedInstructionError> {
    if let LegalizedScalarInstructionKind::BoundarySettlement(settlement) = &row.kind {
        if row.result.is_some()
            || row.operation != settlement.operation
            || row.effect != settlement.effect
            || row.ownership != settlement.ownership
            || row.fuel != settlement.fuel
        {
            return Err(invalid());
        }
        let instruction_index = builder
            .instructions
            .len()
            .checked_sub(block_start)
            .ok_or_else(invalid)?;
        builder
            .transport
            .settlements
            .push(SelectedBoundarySettlement {
                block,
                instruction_index: instruction_index.try_into().map_err(|_| invalid())?,
                settlement: settlement.clone(),
            });
        return Ok(true);
    }
    if row.result.is_some() {
        return Ok(false);
    }
    let LegalizedScalarInstructionKind::Call(call) = &row.kind else {
        return Err(invalid());
    };
    let signature = source.structural.as_ref().ok_or_else(invalid)?;
    call.validate_shape().map_err(|_| invalid())?;
    call.validate_source(&row.ownership)
        .map_err(|_| invalid())?;
    if call.result_placement.is_some()
        || call.arguments.len() != signature.parameters.len()
        || call.call_plan != source.call_plan
    {
        return Err(invalid());
    }
    let mut pointers = Vec::new();
    for (index, argument) in call.arguments.iter().enumerate() {
        let LegalizedScalarArgument::Structural { semantic, target } = argument else {
            return Err(invalid());
        };
        let parameter = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == semantic.place)
            .ok_or_else(invalid)?;
        if semantic.access != StructuralAccess::Owned
            || !semantic.path.is_empty()
            || target.place != semantic.place
            || target.source != parameter.target.placement
            || target.destination != call.call_plan.parameters[index]
            || target.source_byte_offset != 0
        {
            return Err(invalid());
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
            return Err(invalid());
        };
        let slot = OutgoingArgumentSlotId {
            operation: row.operation,
            argument_index: index.try_into().map_err(|_| invalid())?,
        };
        builder.transport.slots.push(SelectedOutgoingArgumentSlot {
            id: slot,
            byte_size: u32::from(*byte_size),
            alignment: *alignment,
            abi_stack_byte_offset: *offset,
        });
        let input = builder
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == semantic.place)
            .map(|(_, register)| *register)
            .ok_or_else(invalid)?;
        for byte_offset in [0, 8] {
            let value = transport_register(builder, semantic.place, byte_offset)?;
            memory(
                builder,
                row,
                semantic.place,
                byte_offset,
                8,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            builder.emit(
                SelectedInstructionKind::Load64 { byte_offset },
                builder.constraints.keys.load64.ok_or_else(invalid)?,
                &[input, value],
                provenance(row),
            )?;
            memory(
                builder,
                row,
                semantic.place,
                byte_offset,
                8,
                SelectedMemoryAccessRole::WriteOutgoing { slot },
            )?;
            builder.emit(
                SelectedInstructionKind::Store64 { slot, byte_offset },
                builder.constraints.keys.store64.ok_or_else(invalid)?,
                &[value],
                provenance(row),
            )?;
        }
        let address = transport_register(builder, semantic.place, 0)?;
        memory(
            builder,
            row,
            semantic.place,
            0,
            u32::from(*byte_size),
            SelectedMemoryAccessRole::AddressOutgoing { slot },
        )?;
        builder.emit(
            SelectedInstructionKind::FrameAddress {
                slot,
                byte_offset: 0,
            },
            builder.constraints.keys.frame_address.ok_or_else(invalid)?,
            &[address],
            provenance(row),
        )?;
        pointers.push((
            address,
            environment
                .fixed_register_view(*pointer)
                .ok_or_else(invalid)?,
        ));
    }
    let key = builder.constraints.keys.call_unit.ok_or_else(invalid)?;
    let constraint = row_constraint(builder, key)?;
    if environment.constraint(key) != Some(constraint)
        || constraint.operands.len() != pointers.len()
        || constraint
            .operands
            .iter()
            .zip(&pointers)
            .any(|(operand, (_, view))| {
                operand.access != RegisterOperandAccess::Use || operand.fixed_view != Some(*view)
            })
    {
        return Err(invalid());
    }
    builder.transport.calls.push(SelectedCallContract {
        instruction: SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| invalid())?,
        ),
        operation: row.operation,
        call: call.clone(),
        effect: row.effect,
        ownership: row.ownership.clone(),
    });
    builder.emit(
        SelectedInstructionKind::CallUnit {
            callee: call.callee,
        },
        key,
        &pointers
            .iter()
            .map(|(register, _)| *register)
            .collect::<Vec<_>>(),
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            fuel: row.fuel.clone(),
            obligations: call.requirement_obligations.clone(),
            ..Default::default()
        },
    )?;
    Ok(true)
}

fn row_constraint<'a>(
    builder: &'a Builder<'_>,
    key: RegisterConstraintKey,
) -> Result<&'a register_model::RegisterInstructionConstraint, SelectedInstructionError> {
    crate::selection::constraints::row(builder.catalog, key)
}
pub(super) fn byte_observation(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    match row.kind {
        LegalizedScalarInstructionKind::ByteSequenceRead { .. } => byte_sequence_read(builder, row),
        LegalizedScalarInstructionKind::ByteSequenceLength {
            source,
            length_byte_offset,
        } => byte_sequence_length(builder, row, source, length_byte_offset),
        _ => Err(invalid()),
    }
}

fn byte_sequence_read(
    builder: &mut Builder<'_>,
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
        return Err(invalid());
    };
    let definition = row.result.ok_or_else(invalid)?;
    let (_, index_register, _, index_type) = builder.resolve(index).ok_or_else(invalid)?;
    if definition.scalar_type
        != ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?)
        || index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
    {
        return Err(invalid());
    }
    let descriptor = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(invalid)?;
    let pointer = transport_register(builder, source, 0)?;
    memory(
        builder,
        row,
        source,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        builder.constraints.keys.load64.ok_or_else(invalid)?,
        &[descriptor, pointer],
        provenance(row),
    )?;
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    memory(
        builder,
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
    builder.emit(
        SelectedInstructionKind::Load8Indexed,
        builder
            .constraints
            .keys
            .load8_indexed
            .ok_or_else(invalid)?,
        &[pointer, index_register, output],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}

fn byte_sequence_length(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
    length_byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = row.result.ok_or_else(invalid)?;
    if length_byte_offset != 8
        || result.scalar_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
    {
        return Err(invalid());
    }
    let input = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(invalid)?;
    let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
    memory(
        builder,
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
    builder.emit(
        SelectedInstructionKind::Load64 {
            byte_offset: length_byte_offset,
        },
        builder.constraints.keys.load64.ok_or_else(invalid)?,
        &[input, output],
        provenance,
    )?;
    Ok(output)
}

fn provenance(row: &LegalizedScalarInstruction) -> SelectedInstructionProvenance {
    SelectedInstructionProvenance {
        operations: vec![row.operation],
        fuel: Vec::new(),
        ..Default::default()
    }
}
fn memory(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
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
        operation: row.operation,
        place,
        byte_offset,
        byte_count,
        role,
    });
    Ok(())
}
