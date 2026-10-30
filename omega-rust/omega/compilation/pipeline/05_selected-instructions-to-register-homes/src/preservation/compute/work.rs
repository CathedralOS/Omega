use optimization_core::OptimizationWorkUsage;

use super::{
    super::AllocatedCalleeSavedRequirementError,
    state::{DirectTraversal, add},
};

pub(in crate::preservation) fn usage(
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
    let function_block_instruction_operand_writes = extent(traversal)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: add(traversal.function_count, 1)?,
        // The derivation is one linear traversal: its writes are traversal
        // work, counted below; its candidates are the requirements it derives.
        candidates: modified_units,
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

/// The program extent the derivation traverses: every function, block,
/// instruction, operand and unit write, each read once. The pass budget's
/// ceilings scale with it (`OptimizationWorkBudget::for_extent`).
pub(in crate::preservation) fn extent(
    traversal: &DirectTraversal<'_>,
) -> Result<u64, AllocatedCalleeSavedRequirementError> {
    [
        traversal.function_count,
        traversal.block_count,
        traversal.instruction_count,
        traversal.operand_count,
        traversal.write_count,
    ]
    .into_iter()
    .try_fold(0_u64, add)
}
