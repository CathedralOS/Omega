use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{BoundaryBranchError, ValidatedBoundaryBranch, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted strict-ordering conditional-branch terminator with
/// the form its compare's pole operand decides: the `Jump` carrying the
/// selected successor, or the `ConditionalBranch` terminator carrying
/// `ConditionalBranchNonZero` when the ordering collapses to the nonzero
/// condition on the same published flag state. The compare the branch
/// observed stays: its flag definitions remain published for every other
/// reached reader. Every other function, block, instruction, register,
/// call, settlement, and access is retained, and replay independently
/// confirms that.
pub fn fold_selected_boundary_branch(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedBoundaryBranch, BoundaryBranchError> {
    let admitted = admission::admit(source, function_index, branch, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    transformed.functions[function_index].blocks[admitted.block_index].terminator =
        admission::rewritten(&admitted);
    super::validate_boundary_branch_fold(
        source,
        function_index,
        branch,
        environment,
        budget,
        transformed,
    )
}
