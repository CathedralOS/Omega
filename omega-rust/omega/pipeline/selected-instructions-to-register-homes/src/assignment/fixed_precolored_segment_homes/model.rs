use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSourceSegmentId, FixedPrecoloredSplitRequirementPlanIdentity, LiveRangeIdentity,
};

pub use register_homes::FixedPrecoloredSegmentHomePlanIdentity;

pub use register_homes::FixedPrecoloredSegmentHomePolicy;

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

pub use register_homes::FixedPrecoloredSegmentHomeValidationReceipt;

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
