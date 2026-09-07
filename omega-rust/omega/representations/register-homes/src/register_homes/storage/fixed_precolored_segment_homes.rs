//! Durable fixed precolored segment homes data; not validation or allocation authority.

mod identity;
pub use identity::fixed_precolored_segment_home_plan_identity;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSourceSegmentId, FixedPrecoloredSplitRequirementPlanIdentity,
};
use selected_instructions::LiveRangeIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedPrecoloredSegmentHomePlanIdentity(pub(crate) [u8; 32]);

impl FixedPrecoloredSegmentHomePlanIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredSegmentHomePolicy {
    /// Place the most constrained remaining domain, then its lowest viable view.
    MostConstrainedLowestCompatibleViewV1,
}

/// Dense function-local identity for one connector-compatible source domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedPrecoloredHomeDomainId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredSegmentHomePlan {
    pub split_requirements: FixedPrecoloredSplitRequirementPlanIdentity,
    pub fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub policy: FixedPrecoloredSegmentHomePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionFixedPrecoloredSegmentHomes>,
    pub structural_unit_functions: Vec<FunctionFixedPrecoloredSegmentHomes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFixedPrecoloredSegmentHomes {
    pub machine: MachineId,
    pub assignments: Vec<FixedPrecoloredSourceSegmentHome>,
}

/// One register-local source segment mapped into a function-local domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSourceSegmentHome {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub source_segment: FixedPrecoloredSourceSegmentId,
    pub allocation_domain: FixedPrecoloredHomeDomainId,
    pub view: RegisterViewId,
}
