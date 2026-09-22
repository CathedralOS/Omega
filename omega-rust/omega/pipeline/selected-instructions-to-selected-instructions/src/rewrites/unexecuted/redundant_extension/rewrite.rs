use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{RedundantExtensionError, ValidatedRedundantExtension, admission};
use crate::ValidatedSelectedAnalysis;

/// Replace one admitted extension with a `CopyI64` of its input register. The
/// input's producer already writes the bits the extension would compute, so
/// the copy preserves the result exactly; every other function, block,
/// instruction, register, call, settlement, and access is retained, and
/// replay independently confirms that.
pub fn remove_selected_redundant_extension(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    extension: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedRedundantExtension, RedundantExtensionError> {
    let admitted = admission::admit(source, function_index, extension, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    function.blocks[admitted.block_index].instructions[admitted.extension_index] =
        admission::rewritten(&admitted);
    super::validate_redundant_extension_removal(
        source,
        function_index,
        extension,
        environment,
        budget,
        transformed,
    )
}
