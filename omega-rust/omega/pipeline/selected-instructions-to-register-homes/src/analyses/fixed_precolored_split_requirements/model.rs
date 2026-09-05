use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    LiveRangeEdgeConnector, LiveRangeIdentity, LiveRangePoint, VirtualFixedConstraintSite,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedPrecoloredSplitRequirementPlanIdentity(pub(crate) [u8; 32]);

impl FixedPrecoloredSplitRequirementPlanIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredSplitRequirementPolicy {
    /// Partition one-block or exact single-entry fanout source ranges only
    /// when a fixed `Use` makes the accumulated physical-view domain empty.
    FixedUseBoundaryRequirementsV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredSplitRequirementPlan {
    pub fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub policy: FixedPrecoloredSplitRequirementPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionFixedPrecoloredSplitRequirements>,
    pub structural_unit_functions: Vec<FunctionFixedPrecoloredSplitRequirements>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFixedPrecoloredSplitRequirements {
    pub machine: MachineId,
    pub registers: Vec<FixedPrecoloredRegisterSplitRequirements>,
}

/// A complete partition of one original source live range. These are source
/// point domains, not post-transformation live intervals or assigned homes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredRegisterSplitRequirements {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub fragments: Vec<FixedPrecoloredSourceFragmentRequirements>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredSourceFragmentRequirements {
    pub block: SelectedBlockId,
    pub source_start: LiveRangePoint,
    pub source_end: LiveRangePoint,
    pub segments: Vec<FixedPrecoloredSourceSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedPrecoloredSourceSegmentId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredSourceSegment {
    pub id: FixedPrecoloredSourceSegmentId,
    pub start: LiveRangePoint,
    pub end: LiveRangePoint,
    /// Complete sorted physical-view domain shared by every source point in
    /// this segment. This is not a chosen view.
    pub candidates: Vec<RegisterViewId>,
    pub opening: FixedPrecoloredSourceSegmentOpening,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedPrecoloredSourceSegmentOpening {
    SourceRangeStartV1,
    IncomingSourceEdgeV1 {
        connector: LiveRangeEdgeConnector,
    },
    /// The incoming exact-view domain and this fixed use's exact-view domain
    /// are disjoint. This does not prescribe a physical movement strategy:
    /// later policy may qualify an alias, copy, rematerialize, or spill/reload.
    IncompatibleFixedUseDomainBoundaryV1 {
        incoming: Option<LiveRangeEdgeConnector>,
        site: VirtualFixedConstraintSite,
        destination_view: RegisterViewId,
    },
}

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
