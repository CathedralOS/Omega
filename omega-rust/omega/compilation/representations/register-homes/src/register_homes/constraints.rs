//! Allocation candidate policies and exact constraints on physical homes.

pub mod allocator_availability;
pub use allocator_availability::{
    AllocatorAvailabilityIdentity, AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy,
    RegisterClassAvailability, allocator_availability_identity,
};
pub mod allocation_legality;
pub use allocation_legality::{
    AllocationLegalityIdentity, AllocationLegalityPlan, EntryFixedViewTransition,
    FunctionAllocationLegality, VirtualEarlyClobberPointLegality, VirtualLegalitySpan,
    VirtualRegisterAllocationLegality, allocation_legality_function_digest,
    allocation_legality_identity_from_digests,
};
pub mod fixed_precolored_intervals;
pub use fixed_precolored_intervals::{
    FixedPrecoloredInterval, FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredIntervalPolicy, FunctionFixedPrecoloredIntervals,
    fixed_precolored_interval_plan_identity,
};
pub mod fixed_precolored_split_requirements;
pub use fixed_precolored_split_requirements::{
    FixedPrecoloredRegisterSplitRequirements, FixedPrecoloredSourceFragmentRequirements,
    FixedPrecoloredSourceSegment, FixedPrecoloredSourceSegmentId,
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementPlanIdentity, FixedPrecoloredSplitRequirementPolicy,
    FunctionFixedPrecoloredSplitRequirements, fixed_precolored_split_requirement_plan_identity,
};
