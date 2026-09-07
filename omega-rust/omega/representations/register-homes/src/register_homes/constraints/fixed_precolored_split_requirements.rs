//! Durable fixed precolored split requirements data; not validation or allocation authority.

mod identity;
pub use identity::fixed_precolored_split_requirement_plan_identity;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
};
use selected_instructions::{
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
