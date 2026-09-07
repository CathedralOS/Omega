//! Validate selections against the one implemented physical sequence.

use super::OptimizedVerifiedPhysicalPipelineError;
use optimization_core::{OptimizationExecutionPhase, PostTerminalOptimizationSelections};

pub(super) fn validate_physical_selections(
    post_terminal: &PostTerminalOptimizationSelections,
    architecture: target::Architecture,
) -> Result<(), OptimizedVerifiedPhysicalPipelineError> {
    let selections = post_terminal.selections();
    // These phases have no current-data implementation in this sequence.
    // A requested rewrite must reject explicitly, never select another emitter.
    for phase in [
        OptimizationExecutionPhase::AbstractOperations,
        OptimizationExecutionPhase::TargetOperations,
        OptimizationExecutionPhase::PreAllocation,
        OptimizationExecutionPhase::SelectedLowering,
        OptimizationExecutionPhase::PostAllocationMachine,
    ] {
        if !selections.project_phase(phase).is_empty() {
            return Err(OptimizedVerifiedPhysicalPipelineError::UnconsumedPostTerminalPhase(phase));
        }
    }
    selected_instructions_to_register_homes::selected_allocation_recovery_rule(
        &selections.project_phase(OptimizationExecutionPhase::AllocationRecovery),
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationRecoveryRuleCatalog)?;
    resolved_layout_to_resolved_layout::x86_rel8_selected(
        &selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
        architecture,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeLayoutRuleCatalog)?;
    Ok(())
}

#[cfg(test)]
mod tests;
