//! Exact primitive producer identity and executed initialization evidence.
use abstract_operations::{AbstractFunction, AbstractOperation};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstructionKind,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, VirtualRegisterOrigin,
};
use semantic_vocabulary::{OperationId, PlaceId, ScalarType};
use terminal_psi::{StructuralMultiplicity, StructuralTypeShape};

pub(super) fn producer(
    function: &AbstractFunction,
    place: PlaceId,
) -> Option<(
    OperationId,
    terminal_psi::StructuralOperationResult,
    abstract_operations::AbstractResult,
)> {
    let mut matching = function
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AbstractOperation::EstablishPrimitiveLocal {
                psi_operation,
                result,
                value,
            } if result.place == place => Some((*psi_operation, result.clone(), *value)),
            _ => None,
        });
    let result = matching.next()?;
    matching.next().is_none().then_some(result)
}

pub(super) fn primitive_size(scalar: ScalarType) -> Option<u8> {
    match scalar {
        ScalarType::Integer(integer)
            if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            Some((integer.bits() / 8) as u8)
        }
        ScalarType::Boolean => Some(1),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some(4),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some(8),
        _ => None,
    }
}

pub(super) fn establishment(
    function: &AbstractFunction,
    selected: &SelectedFunction,
    place: PlaceId,
) -> Option<(OperationId, u8)> {
    let (operation, result, value) = producer(function, place)?;
    let size = primitive_size(value.scalar_type)?;
    let contract = selected.structural.as_ref()?;
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || contract
            .structural_places
            .iter()
            .filter(|declaration| {
                declaration.id == place
                    && declaration.kind
                        == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer: operation,
                            structural_type: result.structural_type,
                        }
            })
            .count()
            != 1
        || contract
            .structural_types
            .iter()
            .filter(|declaration| {
                declaration.id == result.structural_type
                    && declaration.shape == StructuralTypeShape::PrimitiveScalar(value.scalar_type)
            })
            .count()
            != 1
    {
        return None;
    }
    let slot = LocalStorageSlotId::Structural { operation, place };
    let mut slots = selected
        .local_storage_slots
        .iter()
        .filter(|row| row.id == slot);
    let home = slots.next()?;
    if slots.next().is_some()
        || home.byte_size != u32::from(size)
        || home.alignment != u16::from(size)
    {
        return None;
    }
    let mut accesses = selected
        .memory_accesses
        .iter()
        .filter(|row| row.origin == SelectedMemoryAccessOrigin::Operation(operation));
    let address = accesses.next()?;
    let store = accesses.next()?;
    if accesses.next().is_some()
        || address.place != place
        || address.byte_offset != 0
        || address.byte_count != u32::from(size)
        || address.role != (SelectedMemoryAccessRole::AddressLocal { slot })
        || store.place != place
        || store.byte_offset != 0
        || store.byte_count != u32::from(size)
        || store.role != SelectedMemoryAccessRole::WritePlace
    {
        return None;
    }
    let block = selected.blocks.iter().find(|block| {
        block
            .instructions
            .iter()
            .any(|row| row.id == address.instruction)
    })?;
    let address_position = block
        .instructions
        .iter()
        .position(|row| row.id == address.instruction)?;
    let store_position = block
        .instructions
        .iter()
        .position(|row| row.id == store.instruction)?;
    let address_instruction = &block.instructions[address_position];
    let store_instruction = &block.instructions[store_position];
    let pointer = address_instruction.operands.first()?.virtual_register;
    if address_position >= store_position
        || address_instruction.kind
            != (SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            })
        || address_instruction.operands.len() != 1
        || store_instruction.kind
            != (SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: size,
            })
        || store_instruction.operands.len() != 2
        || store_instruction.operands[0].virtual_register != pointer
        || address_instruction.provenance.operations != [operation]
        || store_instruction.provenance.operations != [operation]
        || !store_instruction.provenance.values.contains(&value.value)
        || !selected.virtual_registers.iter().any(|register| {
            register.id == store_instruction.operands[1].virtual_register
                && register.scalar_type == value.scalar_type
                && matches!(register.origin,
                    VirtualRegisterOrigin::EntryParameter { source_value, .. }
                    | VirtualRegisterOrigin::BlockParameter { source_value, .. }
                    | VirtualRegisterOrigin::InstructionResult { source_value, .. }
                    if source_value == value.value)
        })
        || !selected.virtual_registers.iter().any(|register| {
            register.id == pointer
                && register.origin
                    == (VirtualRegisterOrigin::AbiTransport {
                        instruction: address.instruction,
                        place,
                        byte_offset: 0,
                    })
        })
    {
        return None;
    }
    Some((operation, size))
}
