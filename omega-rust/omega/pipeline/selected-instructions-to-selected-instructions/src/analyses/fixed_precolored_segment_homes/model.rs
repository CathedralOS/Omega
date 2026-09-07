use crate::*;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_model::TargetRegisterEnvironmentIdentity;
use semantic_vocabulary::FuelScheduleIdentity;
use target::NativeTarget;

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
    pub(crate) structural_unit_function_count: usize,
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
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
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
