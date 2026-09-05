use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions_to_register_homes::AbstractSpillAccessConstraintPlanIdentity;
use target::NativeTarget;

pub use register_environment::FrameAbiPreservationConvention;

pub use machine_code::{
    FunctionSpillFrameRequirements, NonAuthoritativeSpillFrameRequirementIdentity,
    NonAuthoritativeSpillFrameRequirementPlan, NonAuthoritativeSpillFrameRequirementPolicy,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonAuthoritativeSpillFrameRequirementReceipt {
    pub(in crate::frame_layout::spill_requirements) identity:
        NonAuthoritativeSpillFrameRequirementIdentity,
    pub(in crate::frame_layout::spill_requirements) abstract_spill_access_constraints:
        AbstractSpillAccessConstraintPlanIdentity,
    pub(in crate::frame_layout::spill_requirements) register_environment:
        TargetRegisterEnvironmentIdentity,
    pub(in crate::frame_layout::spill_requirements) target: NativeTarget,
    pub(in crate::frame_layout::spill_requirements) policy:
        NonAuthoritativeSpillFrameRequirementPolicy,
    pub(in crate::frame_layout::spill_requirements) usage: OptimizationWorkUsage,
    pub(in crate::frame_layout::spill_requirements) function_count: usize,
    pub(in crate::frame_layout::spill_requirements) spill_bearing_function_count: usize,
    pub(in crate::frame_layout::spill_requirements) max_abstract_spill_area_bytes: u64,
    pub(in crate::frame_layout::spill_requirements) max_abstract_spill_area_alignment: u64,
}

impl NonAuthoritativeSpillFrameRequirementReceipt {
    pub const fn identity(self) -> NonAuthoritativeSpillFrameRequirementIdentity {
        self.identity
    }
    pub const fn abstract_spill_access_constraints(
        self,
    ) -> AbstractSpillAccessConstraintPlanIdentity {
        self.abstract_spill_access_constraints
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
    pub const fn policy(self) -> NonAuthoritativeSpillFrameRequirementPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn spill_bearing_function_count(self) -> usize {
        self.spill_bearing_function_count
    }
    pub const fn max_abstract_spill_area_bytes(self) -> u64 {
        self.max_abstract_spill_area_bytes
    }
    pub const fn max_abstract_spill_area_alignment(self) -> u64 {
        self.max_abstract_spill_area_alignment
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNonAuthoritativeSpillFrameRequirements {
    pub(in crate::frame_layout::spill_requirements) plan: NonAuthoritativeSpillFrameRequirementPlan,
    pub(in crate::frame_layout::spill_requirements) receipt:
        NonAuthoritativeSpillFrameRequirementReceipt,
}

impl ValidatedNonAuthoritativeSpillFrameRequirements {
    pub const fn plan(&self) -> &NonAuthoritativeSpillFrameRequirementPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> NonAuthoritativeSpillFrameRequirementReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpillFrameRequirementError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTargetConvention,
    NonCanonicalRequirements,
    UsageMismatch,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for SpillFrameRequirementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "spill-frame requirement planning failed: {self:?}"
        )
    }
}

impl std::error::Error for SpillFrameRequirementError {}
