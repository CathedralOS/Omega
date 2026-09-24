//! Optimizer module role: executable entrance. Exact fixed-view-copy recovery stage.
//!
//! This entrance validates source legality, materializes the requested exact
//! policy, independently replays the copy plan, and only then grants custody.

mod compute;
mod custody;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

#[cfg(any(test, feature = "test-support"))]
pub use test_support::OptimizedFixedViewCopyCustodyFieldForTest;
pub use validation::validate_optimized_fixed_view_copy_custody;

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
use terminal_psi::TerminalPsiIdentity;

pub fn stage_optimized_fixed_view_copies(
    source: StagedOptimizedFixedPrecoloredSegmentHomes,
    policy: FixedViewCopyPolicy,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedFixedViewCopies, OptimizedFixedViewCopyCustodyError> {
    validation::validate_source(&source)?;
    let copies = compute::compute_fixed_view_copies(&source, policy, budget)?;
    let custody = validate_optimized_fixed_view_copy_custody(&source, &copies)?;
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
    pub const fn source_segment_home_stage(&self) -> &StagedOptimizedFixedPrecoloredSegmentHomes {
        &self.source
    }
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
    pub const fn custody(&self) -> StagedOptimizedFixedViewCopyCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedFixedViewCopyCustodyReceipt {
    psi: TerminalPsiIdentity,
    target: target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    manifest: PrePhysicalOptimizationManifestIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: register_model::TargetRegisterEnvironmentIdentity,
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
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
    }
    pub const fn target(self) -> target::NativeTarget {
        self.target
    }
    pub const fn entry(self) -> MachineId {
        self.entry
    }
    pub const fn optimization(self) -> OptimizationIdentityBundleIdentity {
        self.optimization
    }
    pub const fn projection(self) -> OptimizedAbstractPlanProjectionIdentity {
        self.projection
    }
    pub const fn manifest(self) -> PrePhysicalOptimizationManifestIdentity {
        self.manifest
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn register_environment(self) -> register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> register_homes::AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_liveness(self) -> selected_instructions::LivenessIdentity {
        self.source_liveness
    }
    pub const fn source_ranges(self) -> selected_instructions::LiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> register_homes::AllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn fixed_intervals(self) -> register_homes::FixedPrecoloredIntervalPlanIdentity {
        self.fixed_intervals
    }
    pub const fn split_requirements(
        self,
    ) -> register_homes::FixedPrecoloredSplitRequirementPlanIdentity {
        self.split_requirements
    }
    pub const fn segment_homes(self) -> register_homes::FixedPrecoloredSegmentHomePlanIdentity {
        self.segment_homes
    }
    pub const fn transformation(self) -> FixedViewCopyIdentity {
        self.transformation
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn policy(self) -> FixedViewCopyPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn copy_count(self) -> usize {
        self.copy_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedFixedViewCopyCustodyError {
    UpstreamSegmentHomes(OptimizedFixedPrecoloredSegmentHomeCustodyError),
    Materialization(FixedViewCopyError),
    Revalidation(FixedViewCopyError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedFixedViewCopyCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized fixed-view copy staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedFixedViewCopyCustodyError {}
