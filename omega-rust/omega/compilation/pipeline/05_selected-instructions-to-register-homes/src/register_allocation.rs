//! Optimizer module role: executable entrance. Selected instructions to register homes.
//!
//! The route, in order: take the selected X-to-X evidence as produced; a completed
//! selected-lowering or pre-allocation run probes assignment over its final
//! facts — success takes `assignment::transformed` homes while residual
//! runtime-recoverable pressure (`RegisterHomeError::admits_runtime_recovery`)
//! hands the proven run — ledger and completion identity included — to
//! `assignment::runtime_spill`; otherwise an
//! admitted recovery rule takes `assignment::recovery`; otherwise legality is
//! staged and the direct assignment either succeeds into `assignment::baseline`
//! homes, or materializes typed operation-local fixed operands and recovers
//! residual runtime-recoverable pressure in
//! `assignment::runtime_spill`.
//! A declared fixed-view rule whose segment-home probe reports a capacity
//! decline — segment pressure or front-end work-budget exhaustion — hands
//! the still-owned legality to `assignment::runtime_spill` before the
//! sequence consumes custody, and residual runtime-recoverable pressure after
//! materialized copies enters it through the post-copy reanalysis — both
//! compositions publish one `RetainedAllocation` like every other branch.
//! The declared active-resident rule composes the same way: its proven
//! rematerialization sweep stays a validated prefix, and residual
//! runtime-recoverable pressure over the rebuilt facts hands
//! that prefix — not the
//! original legality — to `assignment::runtime_spill`.

#[cfg(test)]
mod route_tests;

use crate::assignment::recovery::{
    stage_active_resident_register_allocation, stage_pre_allocation_register_allocation,
    stage_selected_lowering_register_allocation, stage_shared_entry_fixed_view_register_allocation,
};
use optimization_core::{Optimization, OptimizationExecutionPhase};

use crate::{
    AllocationAdmissionError, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedRegisterHomeCustodyError, RetainedAllocation,
};
use selected_instructions_to_selected_instructions::{
    AllocationRecoveryRuleCatalogError, OptimizedAllocationLegalityCustodyError,
    OptimizedFixedPrecoloredSegmentHomeCustodyError, OptimizedFixedViewCopyCustodyError,
    OptimizedSelectedReanalysisError, SelectedInstructionOptimizationEvidence,
    SelectedInstructionOptimizationOutput, selected_allocation_recovery_rule,
    stage_optimized_allocation_legality,
};

/// Execute the exact selected allocation rules and publish one current result.
/// Selected-lowering rewrites have already completed. Assignment consumes their
/// retained proof; pressure recovery remains internal to allocation.
pub fn stage_register_allocation(
    selected: SelectedInstructionOptimizationOutput,
) -> Result<RetainedAllocation, RegisterAllocationError> {
    let ranges = match selected.into_evidence() {
        SelectedInstructionOptimizationEvidence::Identity(ranges) => ranges,
        // A completed rewrite run composes with pressure recovery like every
        // other prefix: assignment probes the run's final facts, success keeps
        // the transformed-homes route, and residual `NoCompatibleHome` or value-local
        // `NoCommonCandidate` hands
        // the run — not a rebuilt legality — to runtime spill so the produced
        // manifest keeps the suite's ledger and completion identity.
        SelectedInstructionOptimizationEvidence::LiteralFolds(run) => {
            return stage_selected_lowering_register_allocation(run);
        }
        SelectedInstructionOptimizationEvidence::PreAllocation(run) => {
            return stage_pre_allocation_register_allocation(run);
        }
    };
    let selections = ranges.selections().clone();
    let recovery = selected_allocation_recovery_rule(
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
        Err(error) if error.admits_runtime_recovery() => {
            let recovered = crate::assignment::runtime_spill::recover(legality)
                .map_err(RegisterAllocationError::RuntimeSpill)?;
            return RetainedAllocation::try_from(recovered)
                .map_err(RegisterAllocationError::Admission);
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
    RetainedAllocation::try_from(homes).map_err(RegisterAllocationError::Admission)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterAllocationError {
    UnsupportedComposition,
    RecoveryCatalog(AllocationRecoveryRuleCatalogError),
    FixedSegments(OptimizedFixedPrecoloredSegmentHomeCustodyError),
    FixedViewCopies(OptimizedFixedViewCopyCustodyError),
    FixedViewHomes(crate::OptimizedPostCopyRegisterHomeCustodyError),
    Reanalysis(OptimizedSelectedReanalysisError),
    Rematerialization(crate::OptimizedActiveResidentRematerializationError),
    RuntimeSpill(crate::RuntimeSpillAllocationError),
    Legality(OptimizedAllocationLegalityCustodyError),
    Homes(OptimizedRegisterHomeCustodyError),
    TransformedHomes(OptimizedPostSelectedLoweringHomeCustodyError),
    PreAllocationHomes(crate::OptimizedPostPreAllocationHomeCustodyError),
    Admission(AllocationAdmissionError),
}

impl std::fmt::Display for RegisterAllocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "register allocation failed: {self:?}")
    }
}

impl std::error::Error for RegisterAllocationError {}
