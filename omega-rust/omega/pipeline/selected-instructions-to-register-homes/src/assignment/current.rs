use super::{stage_active_resident_register_allocation, stage_fixed_view_register_allocation};
use optimization_core::{Optimization, OptimizationExecutionPhase};

use crate::{
    AllocationReplayError, OptimizedAllocationLegalityCustodyError,
    OptimizedLiteralFoldCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedRegisterHomeCustodyError, RetainedAllocation, StagedOptimizedLiveRanges,
    run_selected_lowering_optimizations, stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_frameless_leaf, stage_optimized_register_homes,
    stage_optimized_register_homes_after_selected_lowering,
};

/// Execute the exact selected allocation rules and publish one current result.
/// Availability, rewrites, reanalysis, assignment, and replay belong to this phase.
pub fn stage_register_allocation(
    ranges: StagedOptimizedLiveRanges,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    let selections = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections();
    let recovery = crate::selected_allocation_recovery_rule(
        &selections.project_phase(OptimizationExecutionPhase::AllocationRecovery),
    )
    .map_err(RegisterAllocationError::RecoveryCatalog)?;
    if let Some(rule) = recovery {
        if !selections
            .for_phase(OptimizationExecutionPhase::SelectedLowering)
            .is_empty()
        {
            return Err(RegisterAllocationError::UnsupportedComposition);
        }
        return match rule {
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 => {
                RetainedAllocation::try_from(stage_fixed_view_register_allocation(ranges)?)
                    .map_err(RegisterAllocationError::Replay)
            }
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1 => {
                RetainedAllocation::try_from(stage_active_resident_register_allocation(ranges)?)
                    .map_err(RegisterAllocationError::Replay)
            }
            _ => Err(RegisterAllocationError::UnsupportedComposition),
        };
    }
    if selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .is_empty()
    {
        let legality = stage_optimized_allocation_legality(ranges)
            .map_err(RegisterAllocationError::Legality)?;
        let homes =
            stage_optimized_register_homes(legality).map_err(RegisterAllocationError::Homes)?;
        RetainedAllocation::try_from(homes).map_err(RegisterAllocationError::Replay)
    } else {
        // The currently admitted selected-lowering rules use the frameless-leaf
        // allocation contract. Keep that requirement at their allocation owner.
        let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
            .map_err(RegisterAllocationError::Legality)?;
        let run = run_selected_lowering_optimizations(legality)
            .map_err(RegisterAllocationError::SelectedLowering)?;
        let homes = stage_optimized_register_homes_after_selected_lowering(run)
            .map_err(RegisterAllocationError::TransformedHomes)?;
        RetainedAllocation::try_from(homes).map_err(RegisterAllocationError::Replay)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterAllocationError {
    UnsupportedComposition,
    RecoveryCatalog(crate::AllocationRecoveryRuleCatalogError),
    FixedSegments(crate::OptimizedFixedPrecoloredSegmentHomeCustodyError),
    FixedViewCopies(crate::OptimizedFixedViewCopyCustodyError),
    FixedViewHomes(crate::OptimizedPostCopyRegisterHomeCustodyError),
    Reanalysis(crate::OptimizedSelectedReanalysisError),
    Rematerialization(crate::OptimizedActiveResidentRematerializationError),
    Legality(OptimizedAllocationLegalityCustodyError),
    Homes(OptimizedRegisterHomeCustodyError),
    SelectedLowering(OptimizedLiteralFoldCustodyError),
    TransformedHomes(OptimizedPostSelectedLoweringHomeCustodyError),
    Replay(AllocationReplayError),
}

impl std::fmt::Display for RegisterAllocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "register allocation failed: {self:?}")
    }
}

impl std::error::Error for RegisterAllocationError {}
