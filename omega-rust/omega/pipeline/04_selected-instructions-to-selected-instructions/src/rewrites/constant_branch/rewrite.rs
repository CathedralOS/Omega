use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{ConstantBranchError, ValidatedConstantBranch, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted conditional-branch terminator with the `Jump`
/// carrying its decided successor. The compare the branch observed stays:
/// its flag definitions remain published for every other reached reader,
/// while the folded terminator drops the flag uses because the state it
/// observed decided the edge at compile time. Every other function, block,
/// instruction, register, call, settlement, and access is retained, and
/// replay independently confirms that.
pub fn fold_selected_constant_branch(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedConstantBranch, ConstantBranchError> {
    let admitted = admission::admit(source, function_index, branch, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    transformed.functions[function_index].blocks[admitted.block_index].terminator =
        admission::rewritten(&admitted);
    super::validate_constant_branch_fold(
        source,
        function_index,
        branch,
        environment,
        budget,
        transformed,
    )
}
