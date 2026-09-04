use omega_abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;

use crate::{
    ValidatedLegalizedOperations, ValidatedSelectedInstructions, legalize_target_operations,
    select_instructions,
};
use omega_target_to_register_environment::ValidatedTargetRegisterEnvironment;

use super::constraints::selection_constraints;
use super::model::OptimizedSelectionPipelineError;

pub(super) fn construct_optimized_instruction_selection(
    optimized_target: &ValidatedOptimizedTargetOperations,
    register_environment: &ValidatedTargetRegisterEnvironment,
) -> Result<
    (ValidatedLegalizedOperations, ValidatedSelectedInstructions),
    OptimizedSelectionPipelineError,
> {
    let legalized = legalize_target_operations(
        optimized_target.target_operations(),
        optimized_target.optimized().plan(),
        optimized_target.optimized().unit(),
    )
    .map_err(OptimizedSelectionPipelineError::Legalization)?;
    let selection_constraints = selection_constraints(&legalized, register_environment);
    let selected = select_instructions(
        &legalized,
        &selection_constraints,
        register_environment.physical(),
        register_environment.constraints(),
    )
    .map_err(OptimizedSelectionPipelineError::Selection)?;
    Ok((legalized, selected))
}
