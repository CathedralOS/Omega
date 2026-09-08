//! Rejoin primitive local call arguments to their exact initialized frame residence.
use super::establishment::{establishment, producer};
use crate::function_fragments::{Error, source};
use machine_code::StructuralSourceLocation;
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::{LocalStorageSlotId, SelectedFunction};
use semantic_vocabulary::OperationId;
use terminal_psi::StructuralAccess;

pub(in crate::function_fragments::structural) fn location(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    selected: &SelectedFunction,
    argument: &target_operations::TargetStructuralArgument,
    operation: OperationId,
) -> Result<StructuralSourceLocation, Error> {
    let invalid =
        || Error::Mismatch("primitive local producer, initialization or frame identity differs");
    let (function, _) = source::function(source, selected.machine)?;
    let (producer_operation, size) =
        establishment(function, selected, argument.place).ok_or_else(invalid)?;
    let (_, result, _) = producer(function, argument.place).ok_or_else(invalid)?;
    if producer_operation != operation
        || argument.structural_type != result.structural_type
        || argument.root_structural_type != result.structural_type
        || !argument.path.is_empty()
        || argument.source_byte_offset != 0
        || !matches!(
            argument.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        )
        || argument.shape
            != calling_conventions::ValueShape::borrowed_reference(u16::from(size), u16::from(size))
        || argument.fixed_array_length.is_some()
        || argument.element_stride.is_some()
    {
        return Err(invalid());
    }
    let frame = source::frame(source, selected.machine)?.ok_or_else(invalid)?;
    let slot = LocalStorageSlotId::Structural {
        operation,
        place: argument.place,
    };
    frame_location(
        &frame.local_storage_slots,
        frame.frame_size_bytes,
        slot,
        size,
    )
}

pub(super) fn frame_location(
    homes: &[machine_code::LocalStorageFrameSlot],
    frame_size_bytes: u64,
    slot: LocalStorageSlotId,
    size: u8,
) -> Result<StructuralSourceLocation, Error> {
    let invalid = || Error::Mismatch("primitive local frame identity or extent differs");
    let mut slots = homes.iter().filter(|row| row.id == slot);
    let home = slots.next().ok_or_else(invalid)?;
    if slots.next().is_some()
        || home.size_bytes != u32::from(size)
        || home.alignment_bytes != u16::from(size)
        || !home.frame_offset_bytes.is_multiple_of(u64::from(size))
        || home
            .frame_offset_bytes
            .checked_add(u64::from(size))
            .is_none_or(|end| end > frame_size_bytes)
    {
        return Err(invalid());
    }
    Ok(StructuralSourceLocation::Stack {
        byte_offset: u32::try_from(home.frame_offset_bytes).map_err(|_| Error::Overflow)?,
    })
}
