//! Counts over independently placed section and source fragment records.
use super::TextPlacementError;
use super::conversion::usize_to_u64;
use omega_machine_code::{
    FunctionFragmentEmissionPlan, FunctionFragmentTextSectionStatistics,
    RelocationFreeTextSectionPlacement,
};

pub fn text_section_statistics(
    section: &RelocationFreeTextSectionPlacement,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<FunctionFragmentTextSectionStatistics, TextPlacementError> {
    let mut result = FunctionFragmentTextSectionStatistics::default();
    if fragments.structural_unit_functions.is_empty() {
        result.functions = usize_to_u64(section.functions.len())?;
        result.bytes = section.byte_count;
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
        return Ok(result);
    }
    if !fragments.functions.is_empty()
        || section.functions.len() != fragments.structural_unit_functions.len()
    {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    result.structural_unit_functions = usize_to_u64(fragments.structural_unit_functions.len())?;
    for function in &fragments.structural_unit_functions {
        result.structural_unit_blocks = result
            .structural_unit_blocks
            .checked_add(1)
            .ok_or(TextPlacementError::StatisticsOverflow)?;
        result.structural_unit_bytes = result
            .structural_unit_bytes
            .checked_add(function.byte_count)
            .ok_or(TextPlacementError::StatisticsOverflow)?;
        result.structural_unit_instruction_spans = result
            .structural_unit_instruction_spans
            .checked_add(1 + u64::from(function.block.call.is_some()))
            .ok_or(TextPlacementError::StatisticsOverflow)?;
        result.structural_unit_zero_byte_instruction_spans = result
            .structural_unit_zero_byte_instruction_spans
            .checked_add(u64::from(
                function.block.return_instruction.bytes.is_empty(),
            ))
            .ok_or(TextPlacementError::StatisticsOverflow)?;
        if let Some(call) = &function.block.call {
            result.structural_unit_zero_byte_instruction_spans = result
                .structural_unit_zero_byte_instruction_spans
                .checked_add(u64::from(call.bytes.is_empty()))
                .ok_or(TextPlacementError::StatisticsOverflow)?;
            result.source_internal_machine_fixups = result
                .source_internal_machine_fixups
                .checked_add(1)
                .ok_or(TextPlacementError::StatisticsOverflow)?;
        }
    }
    result.resolved_internal_machine_fixups =
        usize_to_u64(section.resolved_internal_machine_calls.len())?;
    result.remaining_internal_machine_fixups = result
        .source_internal_machine_fixups
        .checked_sub(result.resolved_internal_machine_fixups)
        .ok_or(TextPlacementError::UnresolvedInternalMachineFixups)?;
    if result.structural_unit_bytes != section.byte_count
        || result.remaining_internal_machine_fixups != 0
    {
        return Err(TextPlacementError::UnresolvedInternalMachineFixups);
    }
    Ok(result)
}
