use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_homes::AbstractSpillAccessConstraintPlanIdentity;
use register_model::TargetRegisterEnvironmentIdentity;
use semantic_vocabulary::MachineId;
use target::NativeTarget;

use register_model::FrameAbiPreservationConvention;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NonAuthoritativeSpillFrameRequirementIdentity([u8; 32]);

impl NonAuthoritativeSpillFrameRequirementIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NonAuthoritativeSpillFrameRequirementPolicy {
    AbstractSpillAreaAndPreservationConventionV1,
}

/// Requirements only. This artifact has no selected base, offset, frame size,
/// red-zone placement, shadow space, instruction, fault, unwind, or probing field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonAuthoritativeSpillFrameRequirementPlan {
    pub abstract_spill_access_constraints: AbstractSpillAccessConstraintPlanIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub target: NativeTarget,
    pub policy: NonAuthoritativeSpillFrameRequirementPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionSpillFrameRequirements>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionSpillFrameRequirements {
    pub machine: MachineId,
    pub abstract_spill_area_bytes: u64,
    pub abstract_spill_area_alignment: u64,
    pub abi_preservation_convention: FrameAbiPreservationConvention,
    pub abi_stack_alignment: u16,
    /// ABI capacity fact only; this is never a decision to use the red zone.
    pub abi_red_zone_capacity_bytes: u16,
}
