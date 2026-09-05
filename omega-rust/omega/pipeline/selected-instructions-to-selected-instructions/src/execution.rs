//! Optimizer module role: executable entrance. Selected CFG to selected CFG.

use crate::*;
use optimization_core::OptimizationExecutionPhase;
use target_operations_to_selected_instructions::StagedOptimizedSelectedInstructions;

pub fn optimize_selected_instructions(
    selected: StagedOptimizedSelectedInstructions,
) -> Result<SelectedInstructionOptimizationOutput, SelectedInstructionOptimizationError> {
    let liveness = stage_optimized_liveness(selected)
        .map_err(SelectedInstructionOptimizationError::Liveness)?;
    let ranges = stage_optimized_live_ranges(liveness)
        .map_err(SelectedInstructionOptimizationError::LiveRanges)?;
    optimize_analyzed_selected_instructions(ranges)
}

/// Reuse already checked analysis when driving this phase independently.
/// Identity and nonempty selections both publish the same current-program carrier.
pub fn optimize_analyzed_selected_instructions(
    ranges: StagedOptimizedLiveRanges,
) -> Result<SelectedInstructionOptimizationOutput, SelectedInstructionOptimizationError> {
    let selections = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections();
    let evidence = if selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .is_empty()
    {
        SelectedInstructionOptimizationEvidence::Identity(ranges)
    } else {
        if !selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .is_empty()
        {
            return Err(SelectedInstructionOptimizationError::UnsupportedComposition);
        }
        let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
            .map_err(SelectedInstructionOptimizationError::Legality)?;
        let run = run_selected_lowering_optimizations(legality)
            .map_err(SelectedInstructionOptimizationError::Rewrite)?;
        SelectedInstructionOptimizationEvidence::LiteralFolds(run)
    };
    SelectedInstructionOptimizationOutput::from_evidence(evidence)
}
