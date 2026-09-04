use omega_target_operations_to_selected_instructions::{
    ValidatedLegalizedOperations, ValidatedSelectedInstructions, legalize_target_operations,
    select_instructions,
};

use crate::{
    ValidatedOptimizedTargetOperations, ValidatedTargetRegisterEnvironment,
    baseline_target_register_environment,
};

use super::constraints::selection_constraints;
use super::model::OptimizedSelectionPipelineError;

pub(super) fn construct_optimized_instruction_selection(
    optimized_target: &ValidatedOptimizedTargetOperations,
) -> Result<
    (
        ValidatedTargetRegisterEnvironment,
        ValidatedLegalizedOperations,
        ValidatedSelectedInstructions,
    ),
    OptimizedSelectionPipelineError,
> {
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedSelectionPipelineError::RegisterEnvironment)?;
    let legalized = legalize_target_operations(
        optimized_target.target_operations(),
        optimized_target.optimized().plan(),
        optimized_target.optimized().unit(),
    )
    .map_err(OptimizedSelectionPipelineError::Legalization)?;
    let selection_constraints = selection_constraints(&legalized, &register_environment);
    let selected = select_instructions(
        &legalized,
        &selection_constraints,
        register_environment.physical(),
        register_environment.constraints(),
    )
    .map_err(OptimizedSelectionPipelineError::Selection)?;
    Ok((register_environment, legalized, selected))
}
