use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{DeadStoreEliminationError, ValidatedDeadStoreElimination, admission};
use crate::ValidatedSelectedAnalysis;

/// Remove one admitted `Store`, `StorePacked`, own-storage `Store64`,
/// byte-sequence `Store { 0, 1 }`, or `CopyBytes` destination span whose
/// bytes a later covering store replaces unobserved. The roster drops
/// exactly the dead store's rows — the copy's source read beside its
/// destination span — and the block's boundary settlements shift over the
/// removed ordinal; every other function, block, instruction, register,
/// call, settlement, and access is retained, and replay independently
/// confirms that.
pub fn eliminate_selected_dead_store(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedDeadStoreElimination, DeadStoreEliminationError> {
    let admitted = admission::admit(source, function_index, store, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    // Settlements shift while the block still has its source ordinals.
    function.boundary_settlements = admission::shifted_boundary_settlements(
        admitted.function,
        admitted.block,
        admitted.store_index,
    )?;
    function.blocks[admitted.block_index]
        .instructions
        .remove(admitted.store_index);
    // The dead store's rows drop together — a `CopyBytes` carries its
    // source read beside the destination span. The indices are roster
    // order, so removing highest-first keeps the lower indices valid.
    for &row in admitted.store_accesses.iter().rev() {
        function.memory_accesses.remove(row);
    }
    super::validate_dead_store_elimination(
        source,
        function_index,
        store,
        environment,
        budget,
        transformed,
    )
}
