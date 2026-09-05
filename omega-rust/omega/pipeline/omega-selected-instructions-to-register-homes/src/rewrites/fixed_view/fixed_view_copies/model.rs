use crate::{
    FixedViewCopyError, FixedViewCopyIdentity, FixedViewCopyPolicy, ValidatedFixedViewCopies,
};
use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity, OptimizationWorkUsage,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::StagedOptimizedAllocationLegality;
use crate::{
    OptimizedFixedPrecoloredSegmentHomeCustodyError, StagedOptimizedFixedPrecoloredSegmentHomes,
};

/// Exact named fixed-view copy materialization over the complete source
/// legality chain. It mutates only its private selected-CFG realization and
/// grants no allocation, emission, or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedFixedViewCopies {
    pub(super) source: StagedOptimizedFixedPrecoloredSegmentHomes,
    pub(super) copies: ValidatedFixedViewCopies,
    pub(super) custody: StagedOptimizedFixedViewCopyCustodyReceipt,
}

impl StagedOptimizedFixedViewCopies {
    pub const fn source_segment_home_stage(&self) -> &StagedOptimizedFixedPrecoloredSegmentHomes {
        &self.source
    }
    pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        self.source.source_legality_stage()
    }
    pub const fn copies(&self) -> &ValidatedFixedViewCopies {
        &self.copies
    }
    pub const fn custody(&self) -> StagedOptimizedFixedViewCopyCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedFixedViewCopyCustodyReceipt {
    pub(super) psi: TerminalPsiIdentity,
    pub(super) target: omega_target::NativeTarget,
    pub(super) entry: MachineId,
    pub(super) optimization: OptimizationIdentityBundleIdentity,
    pub(super) projection: OptimizedAbstractPlanProjectionIdentity,
    pub(super) manifest: PrePhysicalOptimizationManifestIdentity,
    pub(super) optimization_unit: OptimizationUnitIdentity,
    pub(super) fuel_schedule: FuelScheduleIdentity,
    pub(super) register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    pub(super) allocator_availability: crate::AllocatorAvailabilityIdentity,
    pub(super) source_selected: SelectedInstructionPlanIdentity,
    pub(super) source_liveness: crate::LivenessIdentity,
    pub(super) source_ranges: crate::LiveRangeIdentity,
    pub(super) source_legality: crate::AllocationLegalityIdentity,
    pub(super) fixed_intervals: crate::FixedPrecoloredIntervalPlanIdentity,
    pub(super) split_requirements: crate::FixedPrecoloredSplitRequirementPlanIdentity,
    pub(super) segment_homes: crate::FixedPrecoloredSegmentHomePlanIdentity,
    pub(super) transformation: FixedViewCopyIdentity,
    pub(super) transformed_selected: SelectedInstructionPlanIdentity,
    pub(super) policy: FixedViewCopyPolicy,
    pub(super) usage: OptimizationWorkUsage,
    pub(super) function_count: usize,
    pub(super) copy_count: usize,
}

impl StagedOptimizedFixedViewCopyCustodyReceipt {
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
    }
    pub const fn target(self) -> omega_target::NativeTarget {
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
    pub const fn register_environment(
        self,
    ) -> omega_register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> crate::AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_liveness(self) -> crate::LivenessIdentity {
        self.source_liveness
    }
    pub const fn source_ranges(self) -> crate::LiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> crate::AllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn fixed_intervals(self) -> crate::FixedPrecoloredIntervalPlanIdentity {
        self.fixed_intervals
    }
    pub const fn split_requirements(self) -> crate::FixedPrecoloredSplitRequirementPlanIdentity {
        self.split_requirements
    }
    pub const fn segment_homes(self) -> crate::FixedPrecoloredSegmentHomePlanIdentity {
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
