use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{BoundaryBooleanError, ValidatedBoundaryBoolean, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted ordering `MaterializeBoolean*` with the
/// materialization its pole operand decides: the `MaterializeI64` of the
/// decided predicate, or the same flag reader re-kinded to
/// `MaterializeBooleanEqual` when the pole collapses the ordering to
/// equality. The compare the boolean observed stays: its flag definitions
/// remain published for every other reached reader. Every other function,
/// block, instruction, register, call, settlement, and access is retained,
/// and replay independently confirms that.
pub fn fold_selected_boundary_boolean(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedBoundaryBoolean, BoundaryBooleanError> {
    let admitted = admission::admit(source, function_index, materialization, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    function.blocks[admitted.block_index].instructions[admitted.materialization_index] =
        admission::rewritten(&admitted);
    super::validate_boundary_boolean_fold(
        source,
        function_index,
        materialization,
        environment,
        budget,
        transformed,
    )
}
