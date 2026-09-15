use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{CopyRemovalError, ValidatedCopyRemoval, admission};
use crate::ValidatedSelectedAnalysis;

/// Remove one admitted `CopyI64` by rebinding every same-block use of its
/// destination to the copied source register. The destination's roster row
/// and the instruction leave the plan together, boundary settlements shift
/// over the removed ordinal, and every other function, block, instruction,
/// register, call, settlement, and access is retained — replay independently
/// confirms exactly that.
pub fn remove_selected_copy(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    copy: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedCopyRemoval, CopyRemovalError> {
    let admitted = admission::admit(source, function_index, copy, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    admission::apply(&admitted, &mut transformed.functions[function_index])?;
    super::validate_copy_removal(
        source,
        function_index,
        copy,
        environment,
        budget,
        transformed,
    )
}
