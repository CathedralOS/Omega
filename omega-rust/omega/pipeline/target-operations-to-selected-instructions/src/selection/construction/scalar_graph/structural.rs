//! Place-backed ABI transport in the ordinary instruction stream.
use super::*;
use crate::selection::byte_view_homes::ByteViewHomes;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use selected_instructions::{
    OutgoingArgumentSlotId, SelectedBoundarySettlement, SelectedCallContract, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedOutgoingArgumentSlot,
};
use semantic_vocabulary::{IntegerType, PlaceId};

mod block_views;
mod entry;
pub(super) use block_views::block_entry;
pub(super) use entry::entry;
mod byte_views;
mod literals;
mod local_storage;
pub(super) use local_storage::fixed_array_argument;
mod case_membership;
mod primitive_locals;
pub(super) use case_membership::observe;
mod scalar_array;
mod scalar_case;
pub(super) use primitive_locals::read;
mod scalar_store;
mod subslice;

pub(super) use byte_views::byte_observation;

#[derive(Default)]
pub(super) struct Transport {
    pub pointers: Vec<(PlaceId, VirtualRegisterId)>,
    views: Vec<ByteViewHomes>,
    pub fragments: Vec<(PlaceId, u32, VirtualRegisterId)>,
    pub slots: Vec<SelectedOutgoingArgumentSlot>,
    pub local_slots: Vec<selected_instructions::SelectedLocalStorageSlot>,
    pub calls: Vec<SelectedCallContract>,
    pub memory: Vec<SelectedMemoryAccess>,
    pub settlements: Vec<SelectedBoundarySettlement>,
}

fn invalid() -> SelectedInstructionError {
    SelectedInstructionError::SourceCustodyMismatch
}

pub(super) fn transport_register(
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

/// End the outgoing fixed-register constraint at a copy of the reference pointer.
pub(super) fn call_pointer(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if !matches!(row.ownership.as_slice(), [optimization_unit::OwnershipEvent::ClaimTransfer(claims)] if claims.is_empty())
        || row.result.is_some_and(|result| {
            !crate::selection::scalar_call_abi::scalar_shape(result.scalar_type)
                .is_some_and(|shape| shape.class == calling_conventions::ValueClass::Integer)
        })
    {
        return Err(invalid());
    }
    let incoming = builder
        .transport
        .pointers
        .iter()
        .find(|(source, _)| *source == place)
        .map(|(_, pointer)| *pointer);
    let input = if let Some(pointer) = incoming {
        pointer
    } else {
        // Borrow the completed aggregate's original home, not ABI fragments.
        let mut homes = builder
            .transport
            .local_slots
            .iter()
            .filter(|home| home.id.structural_place() == Some(place));
        let home = homes.next().ok_or_else(invalid)?.clone();
        if homes.next().is_some() {
            return Err(invalid());
        }
        local_storage::address(builder, row, home.id, 0, home.byte_size, false)?
    };
    let output = transport_register(builder, place, byte_offset)?;
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
                .ok_or_else(invalid)?
        },
        &[input, output],
        SelectedInstructionProvenance::default(),
    )?;
    Ok(output)
}

pub(super) fn operation(
    function: usize,
    source: &LegalizedScalarFunction,
    block: SelectedBlockId,
    block_start: usize,
    row: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<bool, SelectedInstructionError> {
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::ByteSequenceWrite { .. }
    ) {
        byte_views::write(builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishPrimitiveLocal { .. }
            | LegalizedScalarInstructionKind::PrimitiveLocalStore { .. }
    ) {
        primitive_locals::write(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishScalarCase { .. }
    ) {
        scalar_case::establish(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishScalarRecord { .. }
            | LegalizedScalarInstructionKind::EstablishScalarArray { .. }
    ) {
        scalar_array::establish(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
            | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. }
    ) {
        scalar_store::emit(source, row, builder)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
    ) {
        literals::establish(builder, row)?;
        return Ok(true);
    }
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::ByteSequenceSubslice { .. }
    ) {
        subslice::create(source, builder, row)?;
        return Ok(true);
    }
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
                settlement:
                    selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(
                        settlement.clone(),
                    ),
            });
        return Ok(true);
    }
    if row.result.is_some() {
        return Ok(false);
    }
    let LegalizedScalarInstructionKind::Call(call) = &row.kind else {
        return Err(invalid());
    };
    if call.arguments.iter().all(|argument| !matches!(argument, LegalizedScalarArgument::Structural { semantic, target } if semantic.access == StructuralAccess::Owned && crate::selection::scalar_array_input::shape(source, target.structural_type).is_none())) {
        super::unit_call::emit(function, source, row, environment, builder)?;
        return Ok(true);
    }
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
            || target.source
                != target_operations::TargetStructuralArgumentSource::Placement(
                    parameter.target.placement.clone(),
                )
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
                SelectedInstructionKind::Store64 {
                    slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
                    byte_offset,
                },
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
                slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
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
    let key = builder
        .constraints
        .keys
        .call_unit
        .get(call.arguments.len())
        .copied()
        .ok_or_else(invalid)?;
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
        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(row.operation),
        place,
        byte_offset,
        byte_count,
        role,
    });
    Ok(())
}
