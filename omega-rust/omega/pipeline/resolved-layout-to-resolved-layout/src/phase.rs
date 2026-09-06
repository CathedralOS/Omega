//! Optimizer module role: executable entrance. Execute the exact layout phase.

use crate::{stage_optimized_x86_branch_relaxation, x86_rel8_selected};
use optimization_core::{OptimizationPhaseSelections, OptimizationWorkBudget};
use post_allocation_machine_to_post_allocation_machine::StagedOptimizedPostAllocationMachineOptimization;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use register_model::ValidatedPhysicalRegisterModel;
use selected_form_encoding_to_resolved_layout::{
    StagedOptimizedResolvedSelectedFormLayout,
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
};
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

mod error;
mod model;
mod validation;

pub use error::ResolvedLayoutOptimizationError;
pub use model::ResolvedLayoutOptimization;
pub use validation::validate_resolved_layout_optimization;

#[allow(clippy::too_many_arguments)]
pub fn execute_resolved_layout_optimization<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    selections: &OptimizationPhaseSelections,
    budget: OptimizationWorkBudget,
) -> Result<ResolvedLayoutOptimization, ResolvedLayoutOptimizationError> {
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        encoding,
        optimization,
        baseline,
    )
    .map_err(ResolvedLayoutOptimizationError::Baseline)?;
    let enabled = x86_rel8_selected(selections, baseline.target().architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
    if enabled && optimization.is_some() {
        return Err(ResolvedLayoutOptimizationError::UnsupportedComposition);
    }
    let relaxation = if enabled {
        Some(
            stage_optimized_x86_branch_relaxation(
                selected, machine, physical, encoding, baseline, budget,
            )
            .map_err(ResolvedLayoutOptimizationError::Relaxation)?,
        )
    } else {
        None
    };
    let current = match &relaxation {
        Some(relaxation) => relaxation.shared_layout(),
        None => baseline.shared_program(),
    };
    let artifact = ResolvedLayoutOptimization {
        current,
        selections: selections.clone(),
        budget,
        relaxation,
    };
    validate_resolved_layout_optimization(
        selected,
        machine,
        physical,
        encoding,
        optimization,
        baseline,
        selections,
        &artifact,
    )?;
    Ok(artifact)
}
