use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{LiteralArithmeticError, ValidatedLiteralArithmetic, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted register-register `ExactAddI64`/`ExactSubtractI64`
/// with the immediate arithmetic form of its literal operand. The literal's
/// producer already writes the bits the operand reads, so the selected form
/// computes the identical result under the identical obligation and accepted
/// fact; every other function, block, instruction, register, call,
/// settlement, and access is retained, and replay independently confirms
/// that.
pub fn fold_selected_literal_arithmetic(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    instruction: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedLiteralArithmetic, LiteralArithmeticError> {
    let admitted = admission::admit(source, function_index, instruction, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    function.blocks[admitted.block_index].instructions[admitted.instruction_index] =
        admission::rewritten(&admitted);
    super::validate_literal_arithmetic_fold(
        source,
        function_index,
        instruction,
        environment,
        budget,
        transformed,
    )
}
