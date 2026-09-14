use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{LiteralCompareError, ValidatedLiteralCompare, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted register-register `CompareI64` with the immediate or
/// zero compare form of its literal right operand. The literal's producer
/// already writes the bits the subtrahend reads, so the selected form
/// computes the identical condition state; every other function, block,
/// instruction, register, call, settlement, and access is retained, and
/// replay independently confirms that.
pub fn fold_selected_literal_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedLiteralCompare, LiteralCompareError> {
    let admitted = admission::admit(source, function_index, compare, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    function.blocks[admitted.block_index].instructions[admitted.compare_index] =
        admission::rewritten(&admitted);
    super::validate_literal_compare_fold(
        source,
        function_index,
        compare,
        environment,
        budget,
        transformed,
    )
}
