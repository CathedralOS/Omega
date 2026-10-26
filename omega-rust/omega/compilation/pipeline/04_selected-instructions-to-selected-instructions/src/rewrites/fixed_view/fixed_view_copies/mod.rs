//! Optimizer module role: executable entrance. Exact fixed-view-copy recovery stage.
//!
//! This entrance validates source legality, materializes the requested exact
//! policy, independently replays the copy plan, and only then grants custody.

mod compute;
mod custody;
use crate::FixedViewCopyPolicy;
use optimization_core::OptimizationWorkBudget;

use crate::StagedOptimizedFixedPrecoloredSegmentHomes;
use crate::{
    FixedViewCopyError, FixedViewCopyIdentity, OptimizedFixedPrecoloredSegmentHomeCustodyError,
    StagedOptimizedAllocationLegality, ValidatedAllocatorAvailability, ValidatedFixedViewCopies,
};
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationWorkUsage, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target_operations_to_selected_instructions::StagedOptimizedCustodyCore;
use terminal_psi::TerminalPsiIdentity;

pub fn stage_optimized_fixed_view_copies(
    source: StagedOptimizedFixedPrecoloredSegmentHomes,
    policy: FixedViewCopyPolicy,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedFixedViewCopies, OptimizedFixedViewCopyCustodyError> {
    let copies = compute::compute_fixed_view_copies(&source, policy, budget)?;
    let custody = custody::fixed_view_copy_custody_receipt(source.custody(), copies.receipt());
    Ok(StagedOptimizedFixedViewCopies {
        source,
        copies,
        custody,
    })
}

/// Exact named fixed-view copy materialization over the complete source
/// legality chain. It mutates only its private selected-CFG realization and
/// grants no allocation, emission, or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedFixedViewCopies {
    source: StagedOptimizedFixedPrecoloredSegmentHomes,
    copies: ValidatedFixedViewCopies,
    custody: StagedOptimizedFixedViewCopyCustodyReceipt,
}

impl StagedOptimizedFixedViewCopies {
    pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        self.source.source_legality_stage()
    }

    /// The target register environment governing the transformed program.
    /// Copy materialization does not change the admitted environment.
    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        self.source.register_environment()
    }

    /// The environment-derived allocator availability governing the program.
    pub const fn allocator_availability(&self) -> &ValidatedAllocatorAvailability {
        self.source.allocator_availability()
    }

    /// The governing optimizer selections for this admission.
    pub fn selections(&self) -> &OptimizationSelections {
        self.source.selections()
    }

    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.source.budget_per_pass()
    }

    /// The current program: the copy-materialized selected CFG.
    pub const fn copies(&self) -> &ValidatedFixedViewCopies {
        &self.copies
    }
    receipt_accessor_rows!(ref StagedOptimizedFixedViewCopies {
        custody: StagedOptimizedFixedViewCopyCustodyReceipt,
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedFixedViewCopyCustodyReceipt {
    core: StagedOptimizedCustodyCore,
    allocator_availability: register_homes::AllocatorAvailabilityIdentity,
    source_selected: SelectedInstructionPlanIdentity,
    source_liveness: selected_instructions::LivenessIdentity,
    source_ranges: selected_instructions::LiveRangeIdentity,
    source_legality: register_homes::AllocationLegalityIdentity,
    fixed_intervals: register_homes::FixedPrecoloredIntervalPlanIdentity,
    split_requirements: register_homes::FixedPrecoloredSplitRequirementPlanIdentity,
    segment_homes: register_homes::FixedPrecoloredSegmentHomePlanIdentity,
    transformation: FixedViewCopyIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    policy: FixedViewCopyPolicy,
    usage: OptimizationWorkUsage,
    function_count: usize,
    copy_count: usize,
}

impl StagedOptimizedFixedViewCopyCustodyReceipt {
    target_operations_to_selected_instructions::staging_custody_core_accessors!();

    receipt_accessor_rows!(StagedOptimizedFixedViewCopyCustodyReceipt {
        allocator_availability: register_homes::AllocatorAvailabilityIdentity,
        source_selected: SelectedInstructionPlanIdentity,
        source_liveness: selected_instructions::LivenessIdentity,
        source_ranges: selected_instructions::LiveRangeIdentity,
        source_legality: register_homes::AllocationLegalityIdentity,
        transformed_selected: SelectedInstructionPlanIdentity,
        fixed_intervals: register_homes::FixedPrecoloredIntervalPlanIdentity,
        segment_homes: register_homes::FixedPrecoloredSegmentHomePlanIdentity,
        transformation: FixedViewCopyIdentity,
        policy: FixedViewCopyPolicy,
        usage: OptimizationWorkUsage,
        function_count: usize,
        copy_count: usize,
    });
    pub const fn split_requirements(
        self,
    ) -> register_homes::FixedPrecoloredSplitRequirementPlanIdentity {
        self.split_requirements
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedFixedViewCopyCustodyError {
    UpstreamSegmentHomes(OptimizedFixedPrecoloredSegmentHomeCustodyError),
    Materialization(FixedViewCopyError),
    Revalidation(FixedViewCopyError),
    ReceiptMismatch,
}

failed_display!(
    OptimizedFixedViewCopyCustodyError,
    "optimized fixed-view copy staging failed"
);

impl std::error::Error for OptimizedFixedViewCopyCustodyError {}
