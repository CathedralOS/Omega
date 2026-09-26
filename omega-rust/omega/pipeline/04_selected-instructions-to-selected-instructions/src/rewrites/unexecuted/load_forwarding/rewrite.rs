use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{StoredLoadForwardingError, ValidatedStoredLoadForwarding, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted load with the register move or exact-width
/// `ZeroExtend` of the value its place-matched `Store` wrote. The roster
/// drops exactly the load's read row; every other function, block,
/// instruction, register, call, settlement, and access is retained, and
/// replay independently confirms that.
pub fn forward_selected_stored_load(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    load: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedStoredLoadForwarding, StoredLoadForwardingError> {
    let admitted = admission::admit(source, function_index, load, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    function.blocks[admitted.block_index].instructions[admitted.load_index] =
        admission::forwarded(&admitted);
    function.memory_accesses.remove(admitted.load_access);
    super::validate_stored_load_forwarding(
        source,
        function_index,
        load,
        environment,
        budget,
        transformed,
    )
}
