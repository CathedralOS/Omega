//! Project the returning-call envelope from selected instructions and its
//! allocated private home. No tail-of-frame convention or shared scratch slot
//! is inferred: the boundary operation owns both endpoints and the storage.

use machine_code::{
    Aarch64ForeignCallFloatingControlRecord, FunctionTargetFrameLayout, PlacedFunctionFragment,
    X86ForeignCallFloatingControlRecord,
};
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedFunction, SelectedNormalizedForeignCall,
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
) -> Result<FloatingControlCustody, &'static str> {
    let operation = record
        .invocation_site
        .operation()
        .ok_or("floating controls have no source operation")?;
    let slot = LocalStorageSlotId::Boundary { operation };
    let transport = selected_instructions::calls::normalized_foreign::return_transport::resolve(
        function, block, record,
    )
    .ok_or("foreign result transport has no exact control restore")?;
    let call_position = transport.call_position;
    let restore_position = transport
        .restore_position
        .ok_or("foreign control restore is absent")?;
    let save_position =
        selected_instructions::calls::normalized_foreign::return_transport::save_position(
            function,
            block,
            record,
            call_position,
        )
        .ok_or(
            "foreign control save envelope contains unrelated effects or invalid private transport",
        )?;
    let save = &block.instructions[save_position];
    let restore = block
        .instructions
        .get(restore_position)
        .ok_or("foreign control restore position is outside the block")?;
    let mut homes = function
        .local_storage_slots
        .iter()
        .filter(|home| home.id == slot);
    let home = homes.next().ok_or("foreign control slot is absent")?;
    let mut placements = frame
        .local_storage_slots
        .iter()
        .filter(|home| home.id == slot);
    let placement = placements
        .next()
        .ok_or("foreign control slot has no frame placement")?;
    if homes.next().is_some()
        || placements.next().is_some()
        || home.byte_size != 8
        || home.alignment != 8
        || placement.size_bytes != home.byte_size
        || placement.alignment_bytes != home.alignment
        || placement.frame_offset_bytes < frame.outgoing_abi_area.byte_size
        || !placement.frame_offset_bytes.is_multiple_of(8)
        || placement
            .frame_offset_bytes
            .checked_add(8)
            .ok_or("foreign control frame extent overflows")?
            > frame.frame_size_bytes
    {
        return Err("foreign control slot extent or alignment does not match the frame");
    }
    let saved_slot_byte_offset = u32::try_from(
        placement
            .frame_offset_bytes
            .checked_sub(frame.red_zone_resident_bytes)
            .ok_or("foreign control slot precedes committed frame")?,
    )
    .map_err(|_| "foreign control slot offset is not addressable")?;
    let (save_offset, save_byte_count) = super::fragment_instruction_span(fragment, save.id)
        .ok_or("foreign control or call has no emitted span")?;
    let (restore_offset, restore_byte_count) =
        super::fragment_instruction_span(fragment, restore.id)
            .ok_or("foreign control or call has no emitted span")?;
    let (call_offset, call_byte_count) =
        super::fragment_instruction_span(fragment, record.instruction)
            .ok_or("foreign control or call has no emitted span")?;
    // Every intervening instruction is an authenticated result transport copy.
    // Its placed bytes must cover the whole gap, without hiding other effects.
    let transported_end = block.instructions[call_position + 1..restore_position]
        .iter()
        .try_fold(
            call_offset
                .checked_add(call_byte_count)
                .ok_or("foreign call emitted extent overflows")?,
            |end, copy| {
                let (offset, count) = super::fragment_instruction_span(fragment, copy.id)?;
                (offset == end).then_some(())?;
                offset.checked_add(count)
            },
        )
        .ok_or("foreign result transport bytes are not contiguous before restore")?;
    if save_offset
        .checked_add(save_byte_count)
        .ok_or("foreign control save emitted extent overflows")?
        > call_offset
        || restore_offset != transported_end
    {
        return Err("foreign control save/call/restore emitted spans are out of order");
    }
    let save_offset = usize::try_from(save_offset)
        .map_err(|_| "foreign control emitted span is not addressable")?;
    let save_byte_count = usize::try_from(save_byte_count)
        .map_err(|_| "foreign control emitted span is not addressable")?;
    let restore_offset = usize::try_from(restore_offset)
        .map_err(|_| "foreign control emitted span is not addressable")?;
    let restore_byte_count = usize::try_from(restore_byte_count)
        .map_err(|_| "foreign control emitted span is not addressable")?;
    Ok(match target.architecture {
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
