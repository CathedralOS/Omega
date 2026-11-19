use optimization_core::OptimizationSelections;
use selected_instructions::SelectedProgram;

use crate::{
    AllocatorAvailabilityRequirement, OptimizedAllocationLegalityCustodyError,
    SelectedOptimizationRecord, StagedOptimizedAllocationLegality, StagedOptimizedLiveRanges,
    stage_optimized_allocation_legality,
};

/// Stage 07 hands register allocation the selected program and the
/// analyses it already established for that program. After a rewrite phase
/// that includes allocation legality; after an identity run it stops at
/// live ranges. Allocation is the same procedure either way.
#[derive(Debug)]
pub struct SelectedAllocationInput {
    analyses: AllocationAnalyses,
}

#[derive(Debug)]
enum AllocationAnalyses {
    LiveRanges(StagedOptimizedLiveRanges),
    Legality(StagedOptimizedAllocationLegality),
}

impl SelectedAllocationInput {
    pub const fn with_live_ranges(ranges: StagedOptimizedLiveRanges) -> Self {
        Self {
            analyses: AllocationAnalyses::LiveRanges(ranges),
        }
    }

    pub const fn with_legality(legality: StagedOptimizedAllocationLegality) -> Self {
        Self {
            analyses: AllocationAnalyses::Legality(legality),
        }
    }

    pub const fn selected(&self) -> &SelectedProgram {
        match &self.analyses {
            AllocationAnalyses::LiveRanges(ranges) => ranges.selected(),
            AllocationAnalyses::Legality(legality) => legality.selected(),
        }
    }

    pub fn selections(&self) -> &OptimizationSelections {
        match &self.analyses {
            AllocationAnalyses::LiveRanges(ranges) => ranges.selections(),
            AllocationAnalyses::Legality(legality) => legality.selections(),
        }
    }

    pub const fn record(&self) -> &SelectedOptimizationRecord {
        match &self.analyses {
            AllocationAnalyses::LiveRanges(ranges) => ranges.record(),
            AllocationAnalyses::Legality(legality) => legality.record(),
        }
    }

    /// The legality analysis allocation runs on uses the selected recovery
    /// rule's availability, or all allocatable views in the environment
    /// when no recovery rule is selected. A rewrite's private availability
    /// is reused only when it matches; otherwise its live ranges are kept
    /// and legality is derived for allocation's own required view set.
    pub fn into_legality(
        self,
        required: Option<AllocatorAvailabilityRequirement>,
    ) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError>
    {
        let requirement = required
            .unwrap_or(AllocatorAvailabilityRequirement::AllEnvironmentAllocatableViews);
        match self.analyses {
            AllocationAnalyses::LiveRanges(ranges) => {
                stage_optimized_allocation_legality(ranges, requirement)
            }
            AllocationAnalyses::Legality(legality) => legality.for_availability(requirement),
        }
    }
}
