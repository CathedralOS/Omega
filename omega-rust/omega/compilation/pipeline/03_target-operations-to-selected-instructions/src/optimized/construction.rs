use crate::{
    ValidatedLegalizedOperations, ValidatedSelectedInstructions, legalize_target_operations,
    select_instructions,
};
use register_environment::ValidatedTargetRegisterEnvironment;

use super::constraints::selection_constraints;
use super::input::TargetSelectionInput;
use super::model::OptimizedSelectionPipelineError;

pub(super) fn construct_optimized_instruction_selection(
    input: &TargetSelectionInput<'_>,
    register_environment: &ValidatedTargetRegisterEnvironment,
) -> Result<
    (ValidatedLegalizedOperations, ValidatedSelectedInstructions),
    OptimizedSelectionPipelineError,
> {
    let legalized =
        legalize_target_operations(input.target, input.source, input.unit, input.origins)
            .map_err(OptimizedSelectionPipelineError::Legalization)?;
    let selection_constraints = selection_constraints(&legalized, register_environment);
    let selected = select_instructions(&legalized, &selection_constraints, register_environment)
        .map_err(OptimizedSelectionPipelineError::Selection)?;
    Ok((legalized, selected))
}
