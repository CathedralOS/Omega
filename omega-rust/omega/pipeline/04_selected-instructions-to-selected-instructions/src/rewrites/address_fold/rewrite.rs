use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{AddressFoldError, ValidatedAddressFold, admission};
use crate::ValidatedSelectedAnalysis;

/// Rebind one admitted displacement-carrying instruction's base operand to
/// the `AddressOffset` producer's base and carry the combined displacement.
/// The pointer already reads `base + producer_offset`, so the folded
/// instruction observes the identical bytes at `base + combined`; every
/// other function, block, instruction, register, call, settlement, and
/// access is retained, and replay independently confirms that.
pub fn fold_selected_address(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    access: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedAddressFold, AddressFoldError> {
    let admitted = admission::admit(source, function_index, access, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    function.blocks[admitted.block_index].instructions[admitted.consumer_index] =
        admission::rewritten(&admitted);
    super::validate_address_fold(
        source,
        function_index,
        access,
        environment,
        budget,
        transformed,
    )
}
