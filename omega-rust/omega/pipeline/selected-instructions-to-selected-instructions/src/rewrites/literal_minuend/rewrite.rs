use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{LiteralMinuendError, ValidatedLiteralMinuend, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted register-register `CompareI64` with the immediate or
/// zero compare form of its literal left operand. The literal's producer
/// already writes the bits the minuend reads, so the selected form computes
/// `subtrahend - literal` — the swapped subtraction, whose zero condition is
/// the source's — and admission has proven every reachable flag consumer
/// equality-sensing, so no reader observes the inverted ordering bits. Every
/// other function, block, instruction, register, call, settlement, and
/// access is retained, and replay independently confirms that.
pub fn fold_selected_literal_minuend(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedLiteralMinuend, LiteralMinuendError> {
    let admitted = admission::admit(source, function_index, compare, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    function.blocks[admitted.block_index].instructions[admitted.compare_index] =
        admission::rewritten(&admitted);
    super::validate_literal_minuend_fold(
        source,
        function_index,
        compare,
        environment,
        budget,
        transformed,
    )
}
