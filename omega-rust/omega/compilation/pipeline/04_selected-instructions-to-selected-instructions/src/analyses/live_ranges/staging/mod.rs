//! Optimizer module role: executable entrance. Validated liveness to validated live ranges.
//!
//! [`analyze_live_ranges`](crate::analyze_live_ranges) returns only after its
//! independent checker accepts the ranges; this entrance records their
//! identity beside the liveness receipt it inherits and does not replay
//! either again.

mod compute;
mod custody;

use crate::StagedOptimizedLiveness;
use crate::{LiveRangeError, ValidatedLiveRanges, ValidatedLiveness};
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{LiveRangeIdentity, SelectedInstructionPlanIdentity};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target_operations_to_selected_instructions::{
    StagedOptimizedCustodyCore, ValidatedSelectedInstructions,
};
use terminal_psi::TerminalPsiIdentity;

pub(crate) fn stage_optimized_live_ranges(
    liveness: StagedOptimizedLiveness,
) -> Result<StagedOptimizedLiveRanges, OptimizedLiveRangeCustodyError> {
    let ranges = compute::compute_live_ranges(&liveness)?;
    let custody = custody::live_range_custody_receipt(liveness.custody(), ranges.receipt());
    Ok(StagedOptimizedLiveRanges {
        liveness,
        ranges,
        custody,
    })
}

/// Opt-in CFG-aware live-range staging over complete liveness custody. This
/// grants no splitting, allocation, spill, frame, emission, or publication
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedLiveRanges {
    liveness: StagedOptimizedLiveness,
    ranges: ValidatedLiveRanges,
    custody: StagedOptimizedLiveRangeCustodyReceipt,
}

impl StagedOptimizedLiveRanges {
    /// The producer stage, which owns the optimized-target product later
    /// stages share; ordinary consumers read the current program and
    /// analyses directly.
    pub const fn liveness_stage(&self) -> &StagedOptimizedLiveness {
        &self.liveness
    }

    staged_producer_accessors!(
        liveness; liveness, selected, register_environment, selections, budget_per_pass
    );

    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        &self.ranges
    }

    receipt_accessor_rows!(ref StagedOptimizedLiveRanges {
        custody: StagedOptimizedLiveRangeCustodyReceipt,
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedLiveRangeCustodyReceipt {
    core: StagedOptimizedCustodyCore,
    selected: SelectedInstructionPlanIdentity,
    liveness: selected_instructions::LivenessIdentity,
    ranges: LiveRangeIdentity,
}

impl StagedOptimizedLiveRangeCustodyReceipt {
    target_operations_to_selected_instructions::staging_custody_core_accessors!();

    receipt_accessor_rows!(StagedOptimizedLiveRangeCustodyReceipt {
        selected: SelectedInstructionPlanIdentity,
        liveness: selected_instructions::LivenessIdentity,
        ranges: LiveRangeIdentity,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLiveRangeCustodyError {
    Analysis(LiveRangeError),
}

failed_display!(
    OptimizedLiveRangeCustodyError,
    "optimized live-range staging failed"
);

impl std::error::Error for OptimizedLiveRangeCustodyError {}
