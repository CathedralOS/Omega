use super::{TextPlacementError, add};
use omega_machine_code::{PlacedBlockSpan, PlacedFunctionFragment, PlacedInstructionSpan};
use omega_selected_instructions::{MachineAlternativeKey, SelectedBlockId, SelectedInstructionId};
use psi_core::MachineId;

pub(super) fn function(
    candidate: &PlacedFunctionFragment,
    index: usize,
    machine: MachineId,
    offset: u64,
    count: u64,
) -> Result<(), TextPlacementError> {
    if candidate.source_function_index != index as u64
        || candidate.machine != machine
        || candidate.section_offset != offset
        || candidate.byte_count != count
    {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn block(
    candidate: &PlacedBlockSpan,
    block: SelectedBlockId,
    offset: u64,
    section: u64,
    count: u64,
    extent: u64,
    alignment: u64,
) -> Result<(), TextPlacementError> {
    if !offset.is_multiple_of(alignment) || !count.is_multiple_of(alignment) {
        return Err(TextPlacementError::MisalignedAarch64Span);
    }
    if add(offset, count)? > extent {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    if candidate.block != block
        || candidate.function_offset != offset
        || candidate.section_offset != add(section, offset)?
        || candidate.byte_count != count
    {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn instruction(
    candidate: &PlacedInstructionSpan,
    instruction: SelectedInstructionId,
    alternative: MachineAlternativeKey,
    offset: u64,
    section: u64,
    count: u64,
    extent: u64,
    alignment: u64,
) -> Result<(), TextPlacementError> {
    if !offset.is_multiple_of(alignment) || !count.is_multiple_of(alignment) {
        return Err(TextPlacementError::MisalignedAarch64Span);
    }
    if add(offset, count)? > extent {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    if candidate.instruction != instruction
        || candidate.alternative != alternative
        || candidate.function_offset != offset
        || candidate.section_offset != add(section, offset)?
        || candidate.byte_count != count
    {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    Ok(())
}
