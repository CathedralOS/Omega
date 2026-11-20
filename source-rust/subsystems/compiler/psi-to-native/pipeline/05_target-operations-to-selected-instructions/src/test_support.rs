//! Exercise binding and selection with an independently constructed execution plan.
pub use crate::input::{BoundInstallations, InstallationBindingError};
pub use crate::installation_resources::InstallationRequirements;

use crate::{OptimizedSelectionPipelineError, input::BoundExecutionPlan};

/// Check whether this prepared plan selects with exactly these installations.
pub fn select_instructions_for_plan(
    plan: &legalized_operations::PreparedExecutionPlan,
    installations: &BoundInstallations,
) -> Result<
    crate::instruction_selection::ValidatedSelectedInstructions,
    OptimizedSelectionPipelineError,
> {
    let bound = BoundExecutionPlan::bind(plan, installations)
        .map_err(OptimizedSelectionPipelineError::Binding)?;
    let environment =
        register_environment::baseline_target_register_environment(plan.plan().target)
            .map_err(OptimizedSelectionPipelineError::RegisterEnvironment)?;
    crate::instruction_selection::select_bound_instructions(&bound, &environment)
        .map_err(OptimizedSelectionPipelineError::Selection)
}
