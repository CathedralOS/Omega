//! Independently replay operand addresses, both live copies, and final publication.
//! Capacity bounds writable backing; it never supplies a readable extent.
use super::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedMemoryAccessRole,
    VirtualRegisterId, memory,
};
use super::{
    byte_field_store, byte_views, local_storage, provenance, result as transport_register,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use terminal_psi::control_flow::ByteSequenceOperand;

pub(super) fn establish(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
        result,
        left,
        right,
        capacity_obligation,
        accepted_fact,
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    let capacity = crate::selection::byte_concatenation_input::capacity(function, row)
        .ok_or_else(|| replay.invalid())?;
    if replay
        .transport
        .pointers
        .iter()
        .any(|(place, _)| *place == result.place)
    {
        return Err(replay.invalid());
    }
    let integer = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
    );
    let (_, left_length, _, left_type) = replay
        .resolve(left.length())
        .ok_or_else(|| replay.invalid())?;
    let (_, right_length, _, right_type) = replay
        .resolve(right.length())
        .ok_or_else(|| replay.invalid())?;
    if left_type != integer || right_type != integer {
        return Err(replay.invalid());
    }
    let left_pointer = operand_pointer(function, replay, row, left)?;
    let right_pointer = operand_pointer(function, replay, row, right)?;
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: capacity.checked_add(8).ok_or_else(|| replay.invalid())?,
        alignment: 8,
    });
    let owner = local_storage::address(replay, row, slot, 0, capacity + 8, false)?;
    let payload = transport_register(replay, result.place, 8)?;
    replay.check_instruction(
        SelectedInstructionKind::AddressOffset { byte_offset: 8 },
        replay
            .constraints
            .keys
            .address_offset
            .ok_or_else(|| replay.invalid())?,
        &[owner, payload],
        &provenance(row),
    )?;
    copy(
        function,
        replay,
        row,
        left,
        left_pointer,
        payload,
        left_length,
        capacity,
    )?;
    let right_destination = transport_register(replay, result.place, 8)?;
    replay.check_instruction(
        SelectedInstructionKind::ByteViewAddress,
        replay.constraints.keys.add_i64,
        &[payload, left_length, right_destination],
        &provenance(row),
    )?;
    copy(
        function,
        replay,
        row,
        right,
        right_pointer,
        right_destination,
        right_length,
        capacity,
    )?;
    // The accepted mathematical sum is at most the reconstructed capacity,
    // itself representable as u32, so this u64 addition cannot overflow.
    let length = transport_register(replay, result.place, 0)?;
    let evidence = SelectedInstructionProvenance {
        operations: vec![row.operation],
        values: vec![left.length(), right.length()],
        obligations: vec![*capacity_obligation],
        ..Default::default()
    };
    replay.check_instruction(
        SelectedInstructionKind::ExactAddI64 {
            obligation: *capacity_obligation,
            accepted_fact: *accepted_fact,
        },
        replay.constraints.keys.add_i64,
        &[left_length, right_length, length],
        &evidence,
    )?;
    memory(
        replay,
        row,
        result.place,
        0,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 8,
        },
        replay
            .constraints
            .keys
            .store
            .ok_or_else(|| replay.invalid())?,
        &[owner, length],
        &SelectedInstructionProvenance {
            fuel: row.fuel.clone(),
            ..evidence
        },
    )?;
    replay.transport.pointers.push((result.place, owner));
    Ok(())
}

pub(super) fn operand_pointer(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    operand: &ByteSequenceOperand,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    match operand {
        ByteSequenceOperand::Whole { source, .. } => {
            let pointer = byte_views::backing_pointer(function, replay, row, *source)?;
            if let Some(view) = replay
                .transport
                .views
                .iter()
                .find(|view| view.place == *source)
                .copied()
            {
                let address = transport_register(replay, *source, 0)?;
                replay.check_instruction(
                    SelectedInstructionKind::ByteViewAddress,
                    replay.constraints.keys.add_i64,
                    &[pointer, view.byte_offset, address],
                    &provenance(row),
                )?;
                Ok(address)
            } else {
                Ok(pointer)
            }
        }
        ByteSequenceOperand::Field { source, .. } => {
            let offset =
                crate::selection::byte_concatenation_input::field_offset(function, operand)
                    .ok_or_else(|| replay.invalid())?;
            let root = replay
                .transport
                .pointers
                .iter()
                .find(|(place, _)| *place == source.place)
                .map(|(_, pointer)| *pointer)
                .ok_or_else(|| replay.invalid())?;
            let address = transport_register(replay, source.place, offset)?;
            replay.check_instruction(
                SelectedInstructionKind::AddressOffset {
                    byte_offset: offset,
                },
                replay
                    .constraints
                    .keys
                    .address_offset
                    .ok_or_else(|| replay.invalid())?,
                &[root, address],
                &provenance(row),
            )?;
            Ok(address)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn copy(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    operand: &ByteSequenceOperand,
    source_pointer: VirtualRegisterId,
    destination_pointer: VirtualRegisterId,
    count: VirtualRegisterId,
    capacity: u32,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
        result,
        capacity_obligation,
        accepted_fact,
        ..
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    let cursor = byte_field_store::scratch(replay, 3)?;
    let byte = byte_field_store::scratch(replay, 4)?;
    let offset = match operand {
        ByteSequenceOperand::Field { .. } => {
            crate::selection::byte_concatenation_input::field_offset(function, operand)
                .ok_or_else(|| replay.invalid())?
        }
        ByteSequenceOperand::Whole { source, .. } => {
            if crate::selection::byte_concatenation_input::owner_layout_type(function, *source)
                .is_some()
            {
                8
            } else {
                0
            }
        }
    };
    let roots = match operand {
        ByteSequenceOperand::Whole { source, length } => {
            crate::selection::byte_view_homes::view_backing_roots(
                function,
                &replay.transport.views,
                *source,
                *length,
            )?
        }
        ByteSequenceOperand::Field { .. } => None,
    };
    for (root, length) in roots.unwrap_or_else(|| vec![(operand.source(), operand.length())]) {
        memory(
            replay,
            row,
            root,
            offset,
            0,
            SelectedMemoryAccessRole::ReadByteSpan {
                length,
                obligation: *capacity_obligation,
                accepted_fact: *accepted_fact,
            },
        )?;
    }
    // Alias footprints conservatively cover the possible destination range.
    // Exact replay checks each live CopyBytes count and its runtime start; this
    // may-write footprint neither initializes padding nor grants read authority.
    memory(
        replay,
        row,
        result.place,
        8,
        capacity,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::CopyBytes,
        replay
            .constraints
            .keys
            .copy_bytes
            .ok_or_else(|| replay.invalid())?,
        &[source_pointer, destination_pointer, count, cursor, byte],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![operand.length()],
            obligations: vec![*capacity_obligation],
            ..Default::default()
        },
    )
}
