//! Independent replay of place-backed snapshots, copy accesses and call operands.
use super::*;
use crate::selection::byte_view_homes::ByteViewHomes;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use selected_instructions::{
    OutgoingArgumentSlotId, SelectedBoundarySettlement, SelectedCallContract, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedOutgoingArgumentSlot,
};
use semantic_vocabulary::{IntegerType, PlaceId};

mod byte_views;
mod literals;
mod local_storage;
mod scalar_store;
mod subslice;

pub(super) use byte_views::byte_observation;

#[derive(Default)]
pub(super) struct Transport {
    pointers: Vec<(PlaceId, VirtualRegisterId)>,
    views: Vec<ByteViewHomes>,
    pub slots: Vec<SelectedOutgoingArgumentSlot>,
    pub local_slots: Vec<selected_instructions::SelectedLocalStorageSlot>,
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
    if signature.parameters.is_empty() {
        return if crate::selection::literal_storage_input::accepts(source) {
            Ok(())
        } else {
            Err(replay.invalid())
        };
    }
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
    ) && !crate::structural_unit_input::accepts_write_borrow(
        &source.call_plan,
        &parameters,
        &signature.structural_types,
    ) {
        return Err(replay.invalid());
    }
    for (parameter_index, parameter) in signature.parameters.iter().enumerate() {
        let place = parameter.semantic.place;
        if !source.blocks.iter().flat_map(|block|&block.instructions).any(|row| match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldStore { destination, .. }
            | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { destination, .. } => destination.place == place,
            LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. }
            | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. } => *source == place,
            LegalizedScalarInstructionKind::Call(call)=>call.arguments.iter().any(|argument|matches!(argument,LegalizedScalarArgument::Structural {semantic,..} if semantic.place==place)),_=>false,
        }) {continue;}
        if let Some(abi_stack_byte_offset) =
            crate::structural_reference_input::stack_pointer_offset(&parameter.target.placement)
        {
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

/// Reconstruct the outgoing pointer from the independently replayed entry copy.
pub(super) fn call_pointer(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?;
    if !matches!(row.ownership.as_slice(), [optimization_unit::OwnershipEvent::ClaimTransfer(claims)] if claims.is_empty())
        || row
            .result
            .is_some_and(|result| result.scalar_type != ScalarType::Integer(integer))
    {
        return Err(replay.invalid());
    }
    let input = replay
        .transport
        .pointers
        .iter()
        .find(|(source, _)| *source == place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(|| replay.invalid())?;
    let output = result(replay, place, byte_offset)?;
    replay.check_instruction(
        if byte_offset == 0 {
            SelectedInstructionKind::CopyI64
        } else {
            SelectedInstructionKind::AddressOffset { byte_offset }
        },
        if byte_offset == 0 {
            replay.constraints.keys.copy_i64
        } else {
            replay
                .constraints
                .keys
                .address_offset
                .ok_or_else(|| replay.invalid())?
        },
        &[input, output],
        &SelectedInstructionProvenance::default(),
    )?;
    Ok(output)
}

pub(super) fn operation(
    source: &LegalizedScalarFunction,
    node: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
) -> Result<bool, SelectedInstructionError> {
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
            | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. }
    ) {
        scalar_store::validate(source, node, replay)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
    ) {
        literals::establish(replay, node)?;
        return Ok(true);
    }
    if matches!(
        node.kind,
        LegalizedScalarInstructionKind::ByteSequenceSubslice { .. }
    ) {
        subslice::create(source, replay, node)?;
        return Ok(true);
    }
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
                settlement:
                    selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(
                        settlement.clone(),
                    ),
            });
        return Ok(true);
    }
    if node.result.is_some() {
        return Ok(false);
    }
    let LegalizedScalarInstructionKind::Call(call) = &node.kind else {
        return Err(replay.invalid());
    };
    if call.arguments.iter().all(|argument| !matches!(argument, LegalizedScalarArgument::Structural { semantic, .. } if semantic.access == StructuralAccess::Owned)) {
        super::unit_call::validate(source, node, environment, replay)?;
        return Ok(true);
    }
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
            || target.source
                != target_operations::TargetStructuralArgumentSource::Placement(
                    parameter.target.placement.clone(),
                )
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
                SelectedInstructionKind::Store64 {
                    slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
                    byte_offset,
                },
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
                slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
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
        .get(call.arguments.len())
        .copied()
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
