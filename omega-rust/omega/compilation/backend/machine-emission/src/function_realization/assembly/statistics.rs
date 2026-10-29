use super::super::FunctionRelativeOptimizationRealizationStatistics;
use super::super::error::FunctionRelativeOptimizationRealizationError;
use machine_code::ResolvedMachineLayout;

pub(crate) fn function_relative_statistics(
    layout: &ResolvedMachineLayout,
) -> Result<
    FunctionRelativeOptimizationRealizationStatistics,
    FunctionRelativeOptimizationRealizationError,
> {
    let count = |value: usize| {
        u64::try_from(value)
            .map_err(|_| FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
    };
    // Each total the layout already enumerates counts itself; the per-function
    // byte total is the one sum the layout does not yield.
    let bytes = layout
        .functions()
        .iter()
        .try_fold(0_u64, |total, function| {
            total
                .checked_add(function.byte_count)
                .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)
        })?;
    Ok(FunctionRelativeOptimizationRealizationStatistics {
        functions: count(layout.functions().len())?,
        blocks: count(layout.blocks().count())?,
        instructions: count(layout.instructions().count())?,
        bytes,
        resolved_conditional_branches: count(
            layout
                .instructions()
                .filter(|instruction| {
                    instruction
                        .branch
                        .as_deref()
                        .and_then(machine_code::ResolvedBranchEvidence::as_conditional)
                        .is_some()
                })
                .count(),
        )?,
        unresolved_internal_machine_fixups: count(
            layout
                .instructions()
                .filter(|instruction| instruction.internal_machine_fixup.is_some())
                .count(),
        )?,
    })
}
