use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{StoreMutationMotionError, ValidatedStoreMutationMotion, admission};
use crate::ValidatedSelectedAnalysis;

/// Sink one admitted `Store`, `StorePacked`, or own-storage or staging-slot
/// `Store64` to the latest position that keeps the write ordered before
/// every access that could observe it. The roster rows name
/// the store by instruction identity, so the access roster is retained
/// unchanged; boundary settlements at or after the landing index in a crossed
/// target block shift one ordinal later. Every other function, block,
/// instruction, register, call, and access is retained, and replay
/// independently confirms that.
pub fn sink_selected_store_mutation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedStoreMutationMotion, StoreMutationMotionError> {
    let admitted = admission::admit(source, function_index, store, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    // Settlement ordinals name source positions; remap them while the block
    // vectors still hold the source ordinals.
    function.boundary_settlements = admission::shifted_boundary_settlements(
        admitted.function,
        admitted.block,
        admitted.store_index,
        admitted.function.blocks[admitted.target_block_index].id,
        admitted.insert_index,
    )?;
    let moved = function.blocks[admitted.block_index]
        .instructions
        .remove(admitted.store_index);
    function.blocks[admitted.target_block_index]
        .instructions
        .insert(admitted.insert_index, moved);
    super::validate_store_mutation_motion(
        source,
        function_index,
        store,
        environment,
        budget,
        transformed,
    )
}
