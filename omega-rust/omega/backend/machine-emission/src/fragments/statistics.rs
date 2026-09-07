//! Count current fragment records without producing or admitting a manifest.

use machine_code::{FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan};

use machine_code::FunctionFragmentEmissionStatistics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFragmentStatisticsOverflow;

impl std::fmt::Display for FunctionFragmentStatisticsOverflow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("function-fragment statistics overflow")
    }
}

impl std::error::Error for FunctionFragmentStatisticsOverflow {}

pub fn function_fragment_emission_statistics(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<FunctionFragmentEmissionStatistics, FunctionFragmentStatisticsOverflow> {
    let mut result = FunctionFragmentEmissionStatistics {
        functions: u64::try_from(fragments.functions.len())
            .map_err(|_| FunctionFragmentStatisticsOverflow)?,
        ..FunctionFragmentEmissionStatistics::default()
    };
    for function in &fragments.functions {
        result.bytes = result
            .bytes
            .checked_add(function.byte_count)
            .ok_or(FunctionFragmentStatisticsOverflow)?;
        result.blocks = result
            .blocks
            .checked_add(
                u64::try_from(function.blocks.len())
                    .map_err(|_| FunctionFragmentStatisticsOverflow)?,
            )
            .ok_or(FunctionFragmentStatisticsOverflow)?;
        for block in &function.blocks {
            result.instruction_spans = result
                .instruction_spans
                .checked_add(
                    u64::try_from(block.instructions.len())
                        .map_err(|_| FunctionFragmentStatisticsOverflow)?,
                )
                .ok_or(FunctionFragmentStatisticsOverflow)?;
            for row in &block.instructions {
                result.zero_byte_instruction_spans += u64::from(row.bytes.is_empty());
                result.resolved_conditional_branches += u64::from(
                    row.branch
                        .as_deref()
                        .and_then(machine_code::FunctionFragmentBranchEvidence::as_conditional)
                        .is_some(),
                );
                result.unresolved_internal_machine_fixups = result
                    .unresolved_internal_machine_fixups
                    .checked_add(u64::from(row.internal_machine_fixup.is_some()))
                    .ok_or(FunctionFragmentStatisticsOverflow)?;
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
                        .ok_or(FunctionFragmentStatisticsOverflow)?;
                }
                result.logical_fuel_settlements = result
                    .logical_fuel_settlements
                    .checked_add(
                        u64::try_from(fuel).map_err(|_| FunctionFragmentStatisticsOverflow)?,
                    )
                    .ok_or(FunctionFragmentStatisticsOverflow)?;
            }
        }
    }
    Ok(result)
}
