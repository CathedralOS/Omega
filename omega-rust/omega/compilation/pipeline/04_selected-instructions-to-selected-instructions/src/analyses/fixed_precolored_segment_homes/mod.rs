//! Optimizer module role: executable entrance. Fixed/precolored segment-home assignment.
//!
//! This boundary assigns one deterministic physical view to each authenticated
//! source-segment domain. It creates no copy, VReg, instruction, spill, or
//! transformed liveness, and distinct assigned views do not imply movement.

mod compute;
mod error;
mod replay;
mod validation;

pub use error::FixedPrecoloredSegmentHomeError;
use register_homes::FixedPrecoloredSegmentHomePolicy;
pub use validation::validate_fixed_precolored_segment_homes;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomePlanIdentity,
    FixedPrecoloredSplitRequirementPlanIdentity,
};
use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use selected_instructions::LiveRangeIdentity;
use semantic_vocabulary::FuelScheduleIdentity;
use target::NativeTarget;

#[allow(clippy::too_many_arguments)]
pub fn assign_fixed_precolored_segment_homes(
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    fixed: &crate::ValidatedFixedPrecoloredIntervals,
    requirements: &crate::ValidatedFixedPrecoloredSplitRequirements,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: FixedPrecoloredSegmentHomePolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
    let plan = compute::compute(
        ranges,
        legality,
        fixed,
        requirements,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_fixed_precolored_segment_homes(
        ranges,
        legality,
        fixed,
        requirements,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSegmentHomeValidationReceipt {
    pub(crate) identity: FixedPrecoloredSegmentHomePlanIdentity,
    pub(crate) split_requirements: FixedPrecoloredSplitRequirementPlanIdentity,
    pub(crate) fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) target: NativeTarget,
    pub(crate) policy: FixedPrecoloredSegmentHomePolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) domain_count: usize,
    pub(crate) assignment_count: usize,
}

impl FixedPrecoloredSegmentHomeValidationReceipt {
    pub const fn identity(self) -> FixedPrecoloredSegmentHomePlanIdentity {
        self.identity
    }
    pub const fn split_requirements(self) -> FixedPrecoloredSplitRequirementPlanIdentity {
        self.split_requirements
    }
    pub const fn fixed_intervals(self) -> FixedPrecoloredIntervalPlanIdentity {
        self.fixed_intervals
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
    pub const fn policy(self) -> FixedPrecoloredSegmentHomePolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn domain_count(self) -> usize {
        self.domain_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFixedPrecoloredSegmentHomes {
    pub(crate) plan: FixedPrecoloredSegmentHomePlan,
    pub(crate) receipt: FixedPrecoloredSegmentHomeValidationReceipt,
}

impl ValidatedFixedPrecoloredSegmentHomes {
    pub const fn plan(&self) -> &FixedPrecoloredSegmentHomePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> FixedPrecoloredSegmentHomeValidationReceipt {
        self.receipt
    }
}
