use omega_machine_code::{FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan};

use super::super::{FunctionFragmentEmissionError, FunctionFragmentEmissionStatistics};

pub(super) fn compute(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<FunctionFragmentEmissionStatistics, FunctionFragmentEmissionError> {
    let mut result = FunctionFragmentEmissionStatistics {
        functions: u64::try_from(fragments.functions.len())
            .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
        ..FunctionFragmentEmissionStatistics::default()
    };
    for function in &fragments.functions {
        result.bytes = result
            .bytes
            .checked_add(function.byte_count)
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        result.blocks = result
            .blocks
            .checked_add(
                u64::try_from(function.blocks.len())
                    .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
            )
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        for block in &function.blocks {
            result.instruction_spans = result
                .instruction_spans
                .checked_add(
                    u64::try_from(block.instructions.len())
                        .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
                )
                .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
            for row in &block.instructions {
                result.zero_byte_instruction_spans += u64::from(row.bytes.is_empty());
                result.resolved_conditional_branches += u64::from(row.branch.is_some());
                result.unresolved_internal_machine_fixups = result
                    .unresolved_internal_machine_fixups
                    .checked_add(u64::from(row.internal_machine_fixup.is_some()))
                    .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
                let mut fuel = row.provenance.fuel.len();
                if let FunctionFragmentControlProvenance::ConditionalBranch {
                    when_taken,
                    when_fallthrough,
                    ..
                } = &row.control
                {
                    fuel = fuel
                        .checked_add(when_taken.fuel.len())
                        .and_then(|fuel| fuel.checked_add(when_fallthrough.fuel.len()))
                        .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
                }
                result.logical_fuel_settlements = result
                    .logical_fuel_settlements
                    .checked_add(
                        u64::try_from(fuel)
                            .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
                    )
                    .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
            }
        }
    }
    result.structural_unit_functions = u64::try_from(fragments.structural_unit_functions.len())
        .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?;
    for function in &fragments.structural_unit_functions {
        result.structural_unit_blocks = result
            .structural_unit_blocks
            .checked_add(1)
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        result.structural_unit_bytes = result
            .structural_unit_bytes
            .checked_add(function.byte_count)
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        result.structural_unit_instruction_spans = result
            .structural_unit_instruction_spans
            .checked_add(1 + u64::from(function.block.call.is_some()))
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        result.structural_logical_fuel_settlements = result
            .structural_logical_fuel_settlements
            .checked_add(
                u64::try_from(function.block.return_instruction.provenance.fuel.len())
                    .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
            )
            .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        if let Some(call) = &function.block.call {
            result.unresolved_internal_machine_fixups = result
                .unresolved_internal_machine_fixups
                .checked_add(1)
                .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
            result.structural_logical_fuel_settlements = result
                .structural_logical_fuel_settlements
                .checked_add(
                    u64::try_from(call.provenance.fuel.len())
                        .map_err(|_| FunctionFragmentEmissionError::StatisticsOverflow)?,
                )
                .ok_or(FunctionFragmentEmissionError::StatisticsOverflow)?;
        }
    }
    Ok(result)
}
