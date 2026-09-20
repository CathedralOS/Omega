//! Optimizer module role: executable entrance. Selected instructions to register homes.
//!
//! The route, in order: replay the selected X-to-X evidence; a completed
//! selected-lowering run takes `assignment::transformed` homes; otherwise an
//! admitted recovery rule takes `assignment::recovery`; otherwise legality is
//! staged and the direct assignment either succeeds into `assignment::baseline`
//! homes, splits authenticated entry-fixed-view transitions through the
//! shared-source-exit fixed/precolored sequence in `assignment::recovery`, or,
//! on `NoCompatibleHome` pressure, enters `assignment::runtime_spill`.
//! A declared fixed-view rule whose segment-home probe reports a capacity
//! decline — segment pressure or front-end work-budget exhaustion — hands
//! the still-owned legality to `assignment::runtime_spill` before the
//! sequence consumes custody, and residual `NoCompatibleHome` after
//! materialized copies enters it through the post-copy reanalysis — both
//! compositions publish one `RetainedAllocation` like every other branch.
//! The declared active-resident rule composes the same way: its proven
//! rematerialization sweep stays a validated prefix, and residual
//! `NoCompatibleHome` over the rebuilt facts hands that prefix — not the
//! original legality — to `assignment::runtime_spill`.

#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod selected_rewrite_tests;

use crate::assignment::recovery::{
    stage_active_resident_register_allocation,
    stage_leaf_local_fixed_view_register_allocation_composing,
    stage_shared_entry_fixed_view_register_allocation,
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
                let legality = stage_optimized_allocation_legality(ranges)
                    .map_err(RegisterAllocationError::Legality)?;
                stage_shared_entry_fixed_view_register_allocation(legality)
            }
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1 => {
                stage_active_resident_register_allocation(ranges)
            }
            _ => Err(RegisterAllocationError::UnsupportedComposition),
        };
    }
    let legality =
        stage_optimized_allocation_legality(ranges).map_err(RegisterAllocationError::Legality)?;
    let assignment = match crate::assignment::runtime_spill::assign_source(&legality) {
        Ok(homes) => homes,
        Err(crate::RegisterHomeError::UnresolvedEntryTransitions { .. }) => {
            // The transitions recorded in legality are exactly the boundaries
            // the shared-source-exit leg admits — a fan-out of fixed-use
            // sites leaving one segment end shares one dominating copy; the
            // rest copy at their sites. Residual pressure after the copies
            // hands custody to runtime spill inside that sequence. Any other
            // failure class keeps the direct-assignment surface below.
            return stage_leaf_local_fixed_view_register_allocation_composing(legality);
        }
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
