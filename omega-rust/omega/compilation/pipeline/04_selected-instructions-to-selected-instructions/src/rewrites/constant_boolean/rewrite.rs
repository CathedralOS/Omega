use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, ValidatedMachineEffectCatalog};

use super::{ConstantBooleanError, ValidatedConstantBoolean, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted `MaterializeBoolean*` with the `MaterializeI64`
/// carrying its constant predicate outcome. The compare the boolean
/// observed stays: its flag definitions remain published for every other
/// reached reader, while the folded materialization drops the flag uses
/// because the state it observed is compile-time constant. Admission
/// consults the `pair` descriptor table and binds the catalog's effect
/// surface; replay independently confirms the whole edit. Every other
/// function, block, instruction, register, call, settlement, and access is
/// retained.
pub fn fold_selected_constant_boolean(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedConstantBoolean, ConstantBooleanError> {
    let admitted = admission::admit(
        source,
        function_index,
        materialization,
        environment,
        effect_catalog,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    function.blocks[admitted.block_index].instructions[admitted.materialization_index] =
        admission::rewritten(&admitted);
    super::validate_constant_boolean_fold(
        source,
        function_index,
        materialization,
        environment,
        effect_catalog,
        budget,
        transformed,
    )
}
