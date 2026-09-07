//! Counts over independently placed section and source fragment records.
use super::TextPlacementError;
use super::conversion::usize_to_u64;
use machine_code::{
    FunctionFragmentEmissionPlan, FunctionFragmentTextSectionStatistics,
    RelocationFreeTextSectionPlacement,
};

pub fn text_section_statistics(
    section: &RelocationFreeTextSectionPlacement,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<FunctionFragmentTextSectionStatistics, TextPlacementError> {
    let mut result = FunctionFragmentTextSectionStatistics {
        functions: usize_to_u64(section.functions.len())?,
        bytes: section.byte_count,
        ..FunctionFragmentTextSectionStatistics::default()
    };
    for function in &section.functions {
        result.blocks = result
            .blocks
            .checked_add(usize_to_u64(function.blocks.len())?)
            .ok_or(TextPlacementError::StatisticsOverflow)?;
        for block in &function.blocks {
            result.instruction_spans = result
                .instruction_spans
                .checked_add(usize_to_u64(block.instructions.len())?)
                .ok_or(TextPlacementError::StatisticsOverflow)?;
            for row in &block.instructions {
                result.zero_byte_instruction_spans = result
                    .zero_byte_instruction_spans
                    .checked_add(u64::from(row.byte_count == 0))
                    .ok_or(TextPlacementError::StatisticsOverflow)?;
            }
        }
    }
    result.source_internal_machine_fixups = fragments
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.internal_machine_fixup.is_some())
        .count()
        .try_into()
        .map_err(|_| TextPlacementError::StatisticsOverflow)?;
    result.resolved_internal_machine_fixups =
        usize_to_u64(section.resolved_internal_machine_calls.len())?;
    result.remaining_internal_machine_fixups = result
        .source_internal_machine_fixups
        .checked_sub(result.resolved_internal_machine_fixups)
        .ok_or(TextPlacementError::UnresolvedInternalMachineFixups)?;
    if result.remaining_internal_machine_fixups != 0 {
        return Err(TextPlacementError::UnresolvedInternalMachineFixups);
    }
    Ok(result)
}
