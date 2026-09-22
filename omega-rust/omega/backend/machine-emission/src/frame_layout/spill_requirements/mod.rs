//! Optimizer module role: executable entrance. Non-authoritative spill-frame requirements.
//!
//! This join authenticates abstract spill-access custody against the selected
//! register environment, derives requirements, and admits them through an
//! independent replay. It chooses no frame layout or executable operation.

mod compute;
mod custody;
mod identity;
mod replay;
mod validation;

pub use identity::non_authoritative_spill_frame_requirement_identity;
pub use validation::validate_non_authoritative_spill_frame_requirements;

#[cfg(test)]
pub(in crate::frame_layout::spill_requirements) use compute::derive_zero_access_requirement_for_test;
#[cfg(test)]
pub(in crate::frame_layout::spill_requirements) use replay::replay_zero_access_requirement_for_test;

use optimization_core::OptimizationWorkBudget;
use selected_instructions_to_register_homes::unsequenced_spill_stages::ValidatedAbstractSpillAccessConstraints;

use crate::frame_layout::ValidatedTargetRegisterEnvironment;
pub use machine_code::{
    FunctionSpillFrameRequirements, NonAuthoritativeSpillFrameRequirementIdentity,
    NonAuthoritativeSpillFrameRequirementPlan, NonAuthoritativeSpillFrameRequirementPolicy,
};
use optimization_core::OptimizationWorkUsage;
pub use register_environment::FrameAbiPreservationConvention;
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions_to_register_homes::unsequenced_spill_stages::AbstractSpillAccessConstraintPlanIdentity;
use target::NativeTarget;

pub fn stage_non_authoritative_spill_frame_requirements(
    source: &ValidatedAbstractSpillAccessConstraints,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeSpillFrameRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedNonAuthoritativeSpillFrameRequirements, SpillFrameRequirementError> {
    let plan = compute::derive(source, environment, policy, budget)?;
    validate_non_authoritative_spill_frame_requirements(source, environment, plan)
}

#[cfg(test)]
mod tests {
    use super::{derive_zero_access_requirement_for_test, replay_zero_access_requirement_for_test};
    use register_environment::FrameAbiPreservationConvention;

    #[test]
    fn independent_zero_access_rows_retain_neutral_alignment_without_inventing_a_frame() {
        let machine = semantic_vocabulary::MachineId::new(41_991).unwrap();
        let direct = derive_zero_access_requirement_for_test(machine);
        let replayed = replay_zero_access_requirement_for_test(machine);
        assert_eq!(direct, replayed);
        assert_eq!(direct.abstract_spill_area_bytes, 0);
        assert_eq!(direct.abstract_spill_area_alignment, 1);
        assert_eq!(
            direct.abi_preservation_convention,
            FrameAbiPreservationConvention::SystemVAMD64
        );
        assert_eq!(direct.abi_stack_alignment, 16);
        assert_eq!(direct.abi_red_zone_capacity_bytes, 128);
    }
}

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
