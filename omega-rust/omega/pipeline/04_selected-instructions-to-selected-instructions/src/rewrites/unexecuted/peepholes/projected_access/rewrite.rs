use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, ValidatedMachineEffectCatalog};

use super::{ProjectedAccessError, ValidatedProjectedAccess, admission};
use crate::ValidatedSelectedAnalysis;

/// Rebind one admitted access's operand 0 to its `AddressOffset` producer's
/// base register and carry the combined displacement on the access's own
/// kind. The pointer already reads `base + producer_offset`, so the folded
/// instruction performs the identical access at `base + combined` — its
/// declared fault surface retained, not discharged — while the producer
/// stays: its projected register remains published for every other reader.
/// Every other function, block, instruction, register, call, settlement,
/// and access is retained, and replay independently confirms that.
pub fn fold_selected_projected_access(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    access: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedProjectedAccess, ProjectedAccessError> {
    let admitted = admission::admit(
        source,
        function_index,
        access,
        environment,
        effect_catalog,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    transformed.functions[function_index].blocks[admitted.block_index].instructions
        [admitted.position] = admission::rewritten(&admitted);
    super::validate_projected_access_fold(
        source,
        function_index,
        access,
        environment,
        effect_catalog,
        budget,
        transformed,
    )
}
