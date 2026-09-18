//! Proposal rewrite for one admitted condition-materialization pair.
//!
//! This is the proposal half of the declarative pair: admission selected
//! the declared relationship and computed the decided value; the rewrite
//! rebuilds the one body instruction under it. Whether the proposal is
//! legal is for `replay` to decide — it never consults the pair table.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, ValidatedMachineEffectCatalog};

use super::{ConditionMaterializationError, ValidatedConditionMaterialization, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted `MaterializeBoolean*` body instruction with the
/// `MaterializeI64` carrying its decided zero or one under the declared
/// condition-materialization pair. The condition-state producer the
/// materialization observed stays: its flag definitions remain published
/// for every other reached reader, while the folded instruction drops the
/// flag uses because the state it observed decided at compile time. Every
/// other function, block, instruction, terminator, register, call,
/// settlement, and access is retained, and replay independently confirms
/// that.
pub fn fold_selected_condition_materialization(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    consumer: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedConditionMaterialization, ConditionMaterializationError> {
    let admitted = admission::admit(
        source,
        function_index,
        consumer,
        environment,
        effect_catalog,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    transformed.functions[function_index].blocks[admitted.block_index].instructions
        [admitted.position] = admission::rewritten(&admitted);
    super::validate_condition_materialization_fold(
        source,
        function_index,
        consumer,
        environment,
        effect_catalog,
        budget,
        transformed,
    )
}
