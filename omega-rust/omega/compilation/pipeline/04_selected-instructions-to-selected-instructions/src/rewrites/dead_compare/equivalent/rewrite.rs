use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{EquivalentCompareError, ValidatedEquivalentCompare, admission};
use crate::ValidatedSelectedAnalysis;

/// Remove one admitted compare whose published condition-state units the
/// earlier value-equivalent shadows already publish with identical values.
/// The instruction leaves the plan, boundary settlements shift over the
/// removed ordinal, and every other function, block, instruction, register,
/// call, settlement, and access is retained — replay independently confirms
/// exactly that.
pub fn remove_equivalent_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedEquivalentCompare, EquivalentCompareError> {
    let admitted = admission::admit(source, function_index, compare, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    admission::apply(&admitted, &mut transformed.functions[function_index])?;
    super::validate_equivalent_compare(
        source,
        function_index,
        compare,
        environment,
        budget,
        transformed,
    )
}
