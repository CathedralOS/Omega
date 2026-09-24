//! Optimizer module role: executable entrance. Execute the exact layout phase.

use crate::{stage_optimized_x86_branch_relaxation, x86_rel8_selected};
use optimization_core::{OptimizationPhaseSelections, OptimizationWorkBudget};
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use register_model::ValidatedPhysicalRegisterModel;
use selected_form_encoding_to_resolved_layout::{
    StagedOptimizedResolvedSelectedFormLayout, validate_optimized_resolved_selected_form_layout,
};
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

mod error;
mod validation;

use crate::StagedOptimizedX86BranchRelaxation;
pub use error::ResolvedLayoutOptimizationError;
use machine_code::ResolvedMachineLayout;
use std::sync::Arc;
pub use validation::validate_resolved_layout_optimization;

#[allow(clippy::too_many_arguments)]
pub fn execute_resolved_layout_optimization<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    selections: &OptimizationPhaseSelections,
    budget: OptimizationWorkBudget,
) -> Result<ResolvedLayoutOptimization, ResolvedLayoutOptimizationError> {
    validate_optimized_resolved_selected_form_layout(
        selected, machine, physical, encoding, baseline,
    )
    .map_err(ResolvedLayoutOptimizationError::Baseline)?;
    let enabled = x86_rel8_selected(selections, baseline.target().architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
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
        selected, machine, physical, encoding, baseline, selections, &artifact,
    )?;
    Ok(artifact)
}

/// Admission is private to this phase; retained raw data grant no authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLayoutOptimization {
    current: Arc<ResolvedMachineLayout>,
    selections: OptimizationPhaseSelections,
    budget: OptimizationWorkBudget,
    relaxation: Option<StagedOptimizedX86BranchRelaxation>,
}

impl ResolvedLayoutOptimization {
    pub fn layout(&self) -> &ResolvedMachineLayout {
        &self.current
    }
    pub fn shared_layout(&self) -> Arc<ResolvedMachineLayout> {
        Arc::clone(&self.current)
    }
    pub fn relaxation(&self) -> Option<&StagedOptimizedX86BranchRelaxation> {
        self.relaxation.as_ref()
    }
    pub fn selections(&self) -> &OptimizationPhaseSelections {
        &self.selections
    }
    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn relaxation_mut_for_test(&mut self) -> Option<&mut StagedOptimizedX86BranchRelaxation> {
        self.relaxation.as_mut()
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn current_program_mut_for_test(&mut self) -> &mut ResolvedMachineLayout {
        Arc::make_mut(&mut self.current)
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn substitute_shared_layout_for_test(&mut self, current: Arc<ResolvedMachineLayout>) {
        self.current = current;
    }
}
