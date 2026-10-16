//! Rejoin the published control envelope to its admitted selected operations
//! and exact target bytes. Frame geometry was independently checked on the
//! selected-form route; these checks cannot substitute another slot, span or
//! control-state record while preserving that publication binding.

use crate::physical::fragment_publication::FragmentPublicationBinding;
use image_emission::ObjectForeignCall;
use machine_code::PlacedFunctionFragment;
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedFunction, SelectedNormalizedForeignCall,
};
use target::Architecture;

pub(super) fn rejoin(
    publication: &FragmentPublicationBinding,
    function: &SelectedFunction,
    block: &SelectedBlock,
    fragment: &PlacedFunctionFragment,
    record: &SelectedNormalizedForeignCall,
    foreign: &ObjectForeignCall,
) -> bool {
    check(publication, function, block, fragment, record, foreign) == Some(())
}

fn check(
    publication: &FragmentPublicationBinding,
    function: &SelectedFunction,
    block: &SelectedBlock,
    fragment: &PlacedFunctionFragment,
    record: &SelectedNormalizedForeignCall,
    foreign: &ObjectForeignCall,
) -> Option<()> {
    if record.invocation_site.operation().is_none() {
        return (foreign.x86_floating_control.is_none()
            && foreign.aarch64_floating_control.is_none())
        .then_some(());
    }
    let operation = record.invocation_site.operation()?;
    let section = publication.text_section();
    let (target, displacement, save_offset, save_count, restore_offset, restore_count) =
        match section.target.architecture {
            Architecture::Aarch64 => {
                let control = foreign.aarch64_floating_control?;
                if foreign.x86_floating_control.is_some() {
                    return None;
                }
                (
                    control.target,
                    control.saved_slot_byte_offset,
                    control.save_offset,
                    control.save_byte_count,
                    control.restore_offset,
                    control.restore_byte_count,
                )
            }
            Architecture::X86_64 => {
                let control = foreign.x86_floating_control?;
                if foreign.aarch64_floating_control.is_some() {
                    return None;
                }
                (
                    control.target,
                    control.saved_slot_byte_offset,
                    control.save_offset,
                    control.save_byte_count,
                    control.restore_offset,
                    control.restore_byte_count,
                )
            }
        };
    if target != section.target || !displacement.is_multiple_of(8) {
        return None;
    }
    let slot = LocalStorageSlotId::Boundary { operation };
    let mut homes = function
        .local_storage_slots
        .iter()
        .filter(|home| home.id == slot);
    let home = homes.next()?;
    if homes.next().is_some() || home.byte_size != 8 || home.alignment != 8 {
        return None;
    }

    let transport = selected_instructions::calls::normalized_foreign::return_transport::resolve(
        function, block, record,
    )?;
    let call_position = transport.call_position;
    let restore_position = transport.restore_position?;
    let restore = block.instructions.get(restore_position)?;
    let save_position =
        selected_instructions::calls::normalized_foreign::return_transport::save_position(
            function,
            block,
            record,
            call_position,
        )?;
    let save = &block.instructions[save_position];
    let matches_span = |instruction, offset, count| {
        image_emission::fragment_instruction_span(fragment, instruction).is_some_and(
            |(start, size)| {
                usize::try_from(start) == Ok(offset) && usize::try_from(size) == Ok(count)
            },
        )
    };
    let (call_start, call_size) =
        image_emission::fragment_instruction_span(fragment, record.instruction)?;
    let transported_end = block.instructions[call_position + 1..restore_position]
        .iter()
        .try_fold(call_start.checked_add(call_size)?, |end, copy| {
            let (offset, count) = image_emission::fragment_instruction_span(fragment, copy.id)?;
            (offset == end).then_some(())?;
            offset.checked_add(count)
        })?;
    if !matches_span(save.id, save_offset, save_count)
        || !matches_span(restore.id, restore_offset, restore_count)
        || u64::try_from(save_offset.checked_add(save_count)?).ok()? > call_start
        || u64::try_from(restore_offset).ok()? != transported_end
    {
        return None;
    }
    let saved_bytes = section
        .bytes
        .get(save_offset..save_offset.checked_add(save_count)?)?;
    let restored_bytes = section
        .bytes
        .get(restore_offset..restore_offset.checked_add(restore_count)?)?;
    let exact = match target.architecture {
        Architecture::Aarch64 => {
            saved_bytes == isa_aarch64::encode_save_fpcr_to_sp_displacement(displacement).ok()?
                && restored_bytes
                    == isa_aarch64::encode_restore_fpcr_from_sp_displacement(displacement).ok()?
        }
        Architecture::X86_64 => {
            saved_bytes == isa_x86_64::encode_stmxcsr_rsp_displacement(displacement).ok()?
                && restored_bytes
                    == isa_x86_64::encode_ldmxcsr_rsp_displacement(displacement).ok()?
        }
    };
    exact.then_some(())
}
