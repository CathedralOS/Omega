use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, ValidatedMachineEffectCatalog};

use super::{TerminatorPairError, ValidatedTerminatorPair, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted conditional-branch terminator with the `Jump`
/// carrying its decided successor under the declared terminator pair. The
/// condition-state producer the branch observed stays: its flag definitions
/// remain published for every other reached reader, while the folded
/// terminator drops the flag uses because the state it observed decided the
/// edge at compile time. Every other function, block, instruction,
/// register, call, settlement, and access is retained, and replay
/// independently confirms that.
pub fn fold_selected_terminator_pair(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    terminator: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedTerminatorPair, TerminatorPairError> {
    let admitted = admission::admit(
        source,
        function_index,
        terminator,
        environment,
        effect_catalog,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    transformed.functions[function_index].blocks[admitted.block_index].terminator =
        admission::rewritten(&admitted);
    super::validate_terminator_pair_fold(
        source,
        function_index,
        terminator,
        environment,
        effect_catalog,
        budget,
        transformed,
    )
}
