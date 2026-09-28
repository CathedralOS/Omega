use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{DeadCompareError, ValidatedDeadCompare, admission};
use crate::ValidatedSelectedAnalysis;

/// Remove one admitted compare whose published condition-state units all
/// die unread. The instruction leaves the plan, boundary settlements shift
/// over the removed ordinal, and every other function, block, instruction,
/// register, call, settlement, and access is retained — replay
/// independently confirms exactly that.
pub fn remove_dead_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedDeadCompare, DeadCompareError> {
    let admitted = admission::admit(source, function_index, compare, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    admission::apply(&admitted, &mut transformed.functions[function_index])?;
    super::validate_dead_compare(
        source,
        function_index,
        compare,
        environment,
        budget,
        transformed,
    )
}
