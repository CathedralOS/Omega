//! Addressable activation-local storage shared by literal and derived descriptors.
use super::{
    Builder, LegalizedScalarInstruction, PlaceId, SelectedInstructionKind,
    SelectedMemoryAccessRole, VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};

/// Descriptor scratch is operand-owned; its provenance retains the actual
/// backing place by rejoining that operand, not by fabricating a storage owner.
fn storage_origin_place(
    row: &LegalizedScalarInstruction,
    slot: LocalStorageSlotId,
) -> Option<PlaceId> {
    let LocalStorageSlotId::StructuralCallArgument {
        operation,
        argument_index,
    } = slot
    else {
        return slot.structural_place();
    };
    if operation != row.operation {
        return None;
    }
    match &row.kind {
        legalized_operations::LegalizedScalarInstructionKind::Call(call) => {
            let legalized_operations::LegalizedScalarArgument::Structural { semantic, .. } =
                call.arguments.get(usize::try_from(argument_index).ok()?)?
            else {
                return None;
            };
            Some(semantic.place)
        }
        legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(call) => {
            let mut arguments = crate::selection::scalar_call_abi::normalized_foreign::structural_parameter_positions(call)
                .zip(&call.structural_arguments)
                .filter(|(position, _)| *position == argument_index as usize);
            let (_, argument) = arguments.next()?;
            arguments.next().is_none().then_some(argument.place)
        }
        _ => None,
    }
}

pub(in crate::selection::construction::scalar_graph) struct PresentedDescriptor {
    pub address: VirtualRegisterId,
    pub words: Option<[VirtualRegisterId; 2]>,
}

/// Stage the two words of an internal descriptor without copying backing.
/// The caller owns each word's meaning; this operation owns frame transport.
pub(in crate::selection::construction::scalar_graph) fn descriptor(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
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

/// Stage only the declared descriptor; backing remains the original array.
/// Bounded views retain extent in their type and need only the pointer cell.
/// Dynamic views additionally transport the independently checked extent.
pub(in crate::selection::construction::scalar_graph) fn fixed_array_argument(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    argument_index: u32,
    backing: VirtualRegisterId,
    length: u64,
    descriptor_byte_size: u32,
) -> Result<PresentedDescriptor, SelectedInstructionError> {
    if !matches!(descriptor_byte_size, 8 | 16) {
        return Err(SelectedInstructionError::custody());
    }
    let slot = LocalStorageSlotId::StructuralCallArgument {
        operation: row.operation,
        argument_index,
    };
    if descriptor_byte_size == 16 {
        let count = transport_register(builder, place, 8)?;
        builder.emit(
            SelectedInstructionKind::MaterializeI64 {
                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(length)),
            },
            builder.constraints.keys.materialize_i64,
            &[count],
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

/// Stage only the descriptor; the field's live length word and bytes remain in
/// the caller's record. `field` points at the inline storage itself, so the
/// length is loaded there and the data begins eight bytes later — the declared
/// capacity is never the descriptor's length.
pub(in crate::selection::construction::scalar_graph) fn byte_field_argument(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    argument_index: u32,
    field: VirtualRegisterId,
    field_offset: u32,
) -> Result<PresentedDescriptor, SelectedInstructionError> {
    let slot = LocalStorageSlotId::StructuralCallArgument {
        operation: row.operation,
        argument_index,
    };
    let length = transport_register(builder, place, field_offset)?;
    memory(
        builder,
        row,
        place,
        field_offset,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        builder.constraints.keys.load64.ok_or_else(|| invalid())?,
        &[field, length],
        provenance(row),
    )?;
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

pub(super) fn store(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
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
    builder.emit(
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: offset,
        },
        builder.constraints.keys.store64.ok_or_else(|| invalid())?,
        &[value],
        provenance(row),
    )
}

pub(super) fn address(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    slot: LocalStorageSlotId,
    offset: u32,
    byte_count: u32,
    settles_fuel: bool,
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
    if settles_fuel {
        provenance.fuel = row.fuel.clone();
    }
    builder.emit_frame_address(FrameStorageSlotId::Local(slot), register, provenance)?;
    Ok(register)
}
