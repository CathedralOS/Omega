//! Project the returning-call envelope from selected instructions and its
//! allocated private home. No tail-of-frame convention or shared scratch slot
//! is inferred: the boundary operation owns both endpoints and the storage.

use machine_code::{
    Aarch64ForeignCallFloatingControlRecord, FunctionTargetFrameLayout, PlacedFunctionFragment,
    X86ForeignCallFloatingControlRecord,
};
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedFunction, SelectedInstructionKind,
    SelectedNormalizedForeignCall,
};
use target::{Architecture, NativeTarget};

pub(super) struct FloatingControlCustody {
    pub x86: Option<X86ForeignCallFloatingControlRecord>,
    pub aarch64: Option<Aarch64ForeignCallFloatingControlRecord>,
}

pub(super) fn derive(
    target: NativeTarget,
    function: &SelectedFunction,
    block: &SelectedBlock,
    fragment: &PlacedFunctionFragment,
    frame: &FunctionTargetFrameLayout,
    record: &SelectedNormalizedForeignCall,
) -> Option<FloatingControlCustody> {
    let slot = LocalStorageSlotId::Boundary {
        operation: record.operation,
    };
    let call_position = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == record.instruction)?;
    let mut saves = block
        .instructions
        .iter()
        .enumerate()
        .filter(|(_, instruction)| {
            instruction.kind == (SelectedInstructionKind::SaveFloatingControl { slot })
        });
    let (save_position, save) = saves.next()?;
    let restore = block.instructions.get(call_position.checked_add(1)?)?;
    if saves.next().is_some()
        || save_position >= call_position
        || restore.kind != (SelectedInstructionKind::RestoreFloatingControl { slot })
        || block.instructions[save_position..=call_position + 1]
            .iter()
            .any(|instruction| {
                !instruction
                    .provenance
                    .operations
                    .contains(&record.operation)
            })
    {
        return None;
    }
    let mut homes = function
        .local_storage_slots
        .iter()
        .filter(|home| home.id == slot);
    let home = homes.next()?;
    let mut placements = frame
        .local_storage_slots
        .iter()
        .filter(|home| home.id == slot);
    let placement = placements.next()?;
    if homes.next().is_some()
        || placements.next().is_some()
        || home.byte_size != 8
        || home.alignment != 8
        || placement.size_bytes != home.byte_size
        || placement.alignment_bytes != home.alignment
        || placement.frame_offset_bytes < frame.outgoing_abi_area.byte_size
        || !placement.frame_offset_bytes.is_multiple_of(8)
        || placement.frame_offset_bytes.checked_add(8)? > frame.frame_size_bytes
    {
        return None;
    }
    let saved_slot_byte_offset = u32::try_from(
        placement
            .frame_offset_bytes
            .checked_sub(frame.red_zone_resident_bytes)?,
    )
    .ok()?;
    let (save_offset, save_byte_count) = super::instruction_span(fragment, save.id)?;
    let (restore_offset, restore_byte_count) = super::instruction_span(fragment, restore.id)?;
    let (call_offset, call_byte_count) = super::instruction_span(fragment, record.instruction)?;
    if save_offset.checked_add(save_byte_count)? > call_offset
        || restore_offset != call_offset.checked_add(call_byte_count)?
    {
        return None;
    }
    let save_offset = usize::try_from(save_offset).ok()?;
    let save_byte_count = usize::try_from(save_byte_count).ok()?;
    let restore_offset = usize::try_from(restore_offset).ok()?;
    let restore_byte_count = usize::try_from(restore_byte_count).ok()?;
    Some(match target.architecture {
        Architecture::X86_64 => FloatingControlCustody {
            x86: Some(X86ForeignCallFloatingControlRecord {
                target,
                saved_slot_byte_offset,
                save_offset,
                save_byte_count,
                restore_offset,
                restore_byte_count,
            }),
            aarch64: None,
        },
        Architecture::Aarch64 => FloatingControlCustody {
            x86: None,
            aarch64: Some(Aarch64ForeignCallFloatingControlRecord {
                target,
                saved_slot_byte_offset,
                save_offset,
                save_byte_count,
                restore_offset,
                restore_byte_count,
            }),
        },
    })
}
