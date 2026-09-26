use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{RedundantCompareError, ValidatedRedundantCompare, admission};
use crate::ValidatedSelectedAnalysis;

/// Remove one admitted compare whose published condition-state units the
/// earlier flag-equivalent shadow already publishes with identical values.
/// The instruction leaves the plan, boundary settlements shift over the
/// removed ordinal, and every other function, block, instruction, register,
/// call, settlement, and access is retained — replay independently confirms
/// exactly that.
pub fn remove_redundant_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedRedundantCompare, RedundantCompareError> {
    let admitted = admission::admit(source, function_index, compare, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    admission::apply(&admitted, &mut transformed.functions[function_index])?;
    super::validate_redundant_compare(
        source,
        function_index,
        compare,
        environment,
        budget,
        transformed,
    )
}
