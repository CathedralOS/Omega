use crate::*;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_model::TargetRegisterEnvironmentIdentity;
use semantic_vocabulary::FuelScheduleIdentity;
use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSplitRequirementValidationReceipt {
    pub(crate) identity: FixedPrecoloredSplitRequirementPlanIdentity,
    pub(crate) fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) target: NativeTarget,
    pub(crate) policy: FixedPrecoloredSplitRequirementPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) structural_unit_function_count: usize,
    pub(crate) register_count: usize,
    pub(crate) fragment_count: usize,
    pub(crate) source_point_count: usize,
    pub(crate) segment_count: usize,
    pub(crate) incompatible_fixed_use_boundary_count: usize,
}

impl FixedPrecoloredSplitRequirementValidationReceipt {
    pub const fn identity(self) -> FixedPrecoloredSplitRequirementPlanIdentity {
        self.identity
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
    pub const fn policy(self) -> FixedPrecoloredSplitRequirementPolicy {
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
    pub const fn register_count(self) -> usize {
        self.register_count
    }
    pub const fn fragment_count(self) -> usize {
        self.fragment_count
    }
    pub const fn source_point_count(self) -> usize {
        self.source_point_count
    }
    pub const fn segment_count(self) -> usize {
        self.segment_count
    }
    pub const fn incompatible_fixed_use_boundary_count(self) -> usize {
        self.incompatible_fixed_use_boundary_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFixedPrecoloredSplitRequirements {
    pub(crate) plan: FixedPrecoloredSplitRequirementPlan,
    pub(crate) receipt: FixedPrecoloredSplitRequirementValidationReceipt,
}

impl ValidatedFixedPrecoloredSplitRequirements {
    pub const fn plan(&self) -> &FixedPrecoloredSplitRequirementPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> FixedPrecoloredSplitRequirementValidationReceipt {
        self.receipt
    }
}
