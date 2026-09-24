use optimization_core::OptimizationWorkUsage;

use super::{
    super::AllocatedCalleeSavedRequirementError,
    state::{DirectTraversal, add},
};

pub(super) fn usage(
    traversal: &DirectTraversal<'_>,
) -> Result<OptimizationWorkUsage, AllocatedCalleeSavedRequirementError> {
    let modified_units = traversal
        .functions
        .iter()
        .try_fold(0_u64, |total, function| {
            add(
                total,
                u64::try_from(function.modified_units.len())
                    .map_err(|_| AllocatedCalleeSavedRequirementError::WorkOverflow)?,
            )
        })?;
    let function_block_instruction_operand_writes = [
        traversal.function_count,
        traversal.block_count,
        traversal.instruction_count,
        traversal.operand_count,
        traversal.write_count,
    ]
    .into_iter()
    .try_fold(0_u64, add)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: add(traversal.function_count, 1)?,
        candidates: traversal.write_count,
        validation_steps: function_block_instruction_operand_writes,
        commits: [
            1,
            traversal.function_count,
            modified_units,
            traversal.witness_count,
        ]
        .into_iter()
        .try_fold(0_u64, add)?,
        iterations: function_block_instruction_operand_writes,
    })
}
