//! Optimizer module role: executable entrance. Selected instructions to register homes.
//!
//! The route, in order: replay the selected X-to-X evidence; a completed
//! selected-lowering run takes `assignment::transformed` homes; otherwise an
//! admitted recovery rule takes `assignment::recovery`; otherwise legality is
//! staged and the direct assignment either succeeds into `assignment::baseline`
//! homes or, on `NoCompatibleHome` pressure, enters `assignment::runtime_spill`.
//! Every branch publishes one `RetainedAllocation`.

use crate::assignment::recovery::{
    stage_active_resident_register_allocation, stage_fixed_view_register_allocation,
};
use optimization_core::{Optimization, OptimizationExecutionPhase};

use crate::{
    AllocationReplayError, OptimizedAllocationLegalityCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
    RetainedAllocation, stage_optimized_allocation_legality,
};

/// Execute the exact selected allocation rules and publish one current result.
/// Selected-lowering rewrites have already completed. Assignment consumes their
/// retained proof; pressure recovery remains internal to allocation.
pub fn stage_register_allocation(
    selected: crate::SelectedInstructionOptimizationOutput,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    let ranges = match selected
        .into_replayed_evidence()
        .map_err(RegisterAllocationError::SelectedOptimization)?
    {
        crate::SelectedInstructionOptimizationEvidence::Identity(ranges) => ranges,
        crate::SelectedInstructionOptimizationEvidence::LiteralFolds(run) => {
            let homes =
                crate::assignment::transformed::stage_optimized_register_homes_after_selected_lowering(run)
                .map_err(RegisterAllocationError::TransformedHomes)?;
            return RetainedAllocation::try_from(homes).map_err(RegisterAllocationError::Replay);
        }
    };
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
    let legality =
        stage_optimized_allocation_legality(ranges).map_err(RegisterAllocationError::Legality)?;
    let assignment = match crate::assignment::runtime_spill::assign_source(&legality) {
        Ok(homes) => homes,
        Err(crate::RegisterHomeError::NoCompatibleHome { .. }) => {
            let recovered = crate::assignment::runtime_spill::recover(legality)
                .map_err(RegisterAllocationError::RuntimeSpill)?;
            return RetainedAllocation::try_from(recovered)
                .map_err(RegisterAllocationError::Replay);
        }
        Err(error) => {
            return Err(RegisterAllocationError::Homes(
                OptimizedRegisterHomeCustodyError::Assignment(error),
            ));
        }
    };
    let homes =
        crate::assignment::baseline::stage_register_homes_with_assignment(legality, assignment)
            .map_err(RegisterAllocationError::Homes)?;
    RetainedAllocation::try_from(homes).map_err(RegisterAllocationError::Replay)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterAllocationError {
    SelectedOptimization(crate::SelectedInstructionOptimizationError),
    UnsupportedComposition,
    RecoveryCatalog(crate::AllocationRecoveryRuleCatalogError),
    FixedSegments(crate::OptimizedFixedPrecoloredSegmentHomeCustodyError),
    FixedViewCopies(crate::OptimizedFixedViewCopyCustodyError),
    FixedViewHomes(crate::OptimizedPostCopyRegisterHomeCustodyError),
    Reanalysis(crate::OptimizedSelectedReanalysisError),
    Rematerialization(crate::OptimizedActiveResidentRematerializationError),
    RuntimeSpill(crate::RuntimeSpillAllocationError),
    Legality(OptimizedAllocationLegalityCustodyError),
    Homes(OptimizedRegisterHomeCustodyError),
    TransformedHomes(OptimizedPostSelectedLoweringHomeCustodyError),
    Replay(AllocationReplayError),
}

impl std::fmt::Display for RegisterAllocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "register allocation failed: {self:?}")
    }
}

impl std::error::Error for RegisterAllocationError {}
