//! Optimizer module role: stage input. Exact physical-phase selection projection.

use omega_optimization_core::{
    OptimizationExecutionPhase, OptimizationPhaseSelections, PostTerminalOptimizationSelections,
};

use super::OptimizedVerifiedPhysicalPipelineError;

/// One canonical projection of the post-Terminal selection into the physical
/// stages that currently have executable catalogs.
///
/// Earlier post-Terminal phases reject while they have no stage implementation;
/// no physical coordinator may silently ignore them or rediscover a schedule
/// by rescanning the complete selection.
pub(crate) struct PhysicalOptimizationPhaseSelections {
    selected_lowering: OptimizationPhaseSelections,
    allocation_recovery: OptimizationPhaseSelections,
    post_allocation_machine: OptimizationPhaseSelections,
    function_relative_layout: OptimizationPhaseSelections,
}

impl PhysicalOptimizationPhaseSelections {
    pub(crate) fn project(
        post_terminal: &PostTerminalOptimizationSelections,
    ) -> Result<Self, OptimizedVerifiedPhysicalPipelineError> {
        let selections = post_terminal.selections();
        for phase in [
            OptimizationExecutionPhase::AbstractOperations,
            OptimizationExecutionPhase::TargetOperations,
            OptimizationExecutionPhase::PreAllocation,
        ] {
            if !selections.project_phase(phase).is_empty() {
                return Err(
                    OptimizedVerifiedPhysicalPipelineError::UnconsumedPostTerminalPhase(phase),
                );
            }
        }
        Ok(Self {
            selected_lowering: selections
                .project_phase(OptimizationExecutionPhase::SelectedLowering),
            allocation_recovery: selections
                .project_phase(OptimizationExecutionPhase::AllocationRecovery),
            post_allocation_machine: selections
                .project_phase(OptimizationExecutionPhase::PostAllocationMachine),
            function_relative_layout: selections
                .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
        })
    }

    pub(crate) const fn selected_lowering(&self) -> &OptimizationPhaseSelections {
        &self.selected_lowering
    }

    pub(crate) const fn allocation_recovery(&self) -> &OptimizationPhaseSelections {
        &self.allocation_recovery
    }

    pub(crate) const fn post_allocation_machine(&self) -> &OptimizationPhaseSelections {
        &self.post_allocation_machine
    }

    pub(crate) const fn function_relative_layout(&self) -> &OptimizationPhaseSelections {
        &self.function_relative_layout
    }
}
