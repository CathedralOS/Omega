use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::AbstractSpillAccessConstraintPlanIdentity;
use omega_register_model::TargetRegisterEnvironmentIdentity;
use omega_target::NativeTarget;
use psi_core::MachineId;

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

/// Exact target-owned preservation row selected for planning facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameAbiPreservationConvention {
    SystemVAMD64,
    MicrosoftX64,
    Aapcs64,
    DarwinAapcs64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonAuthoritativeSpillFrameRequirementReceipt {
    pub(crate) identity: NonAuthoritativeSpillFrameRequirementIdentity,
    pub(crate) abstract_spill_access_constraints: AbstractSpillAccessConstraintPlanIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) target: NativeTarget,
    pub(crate) policy: NonAuthoritativeSpillFrameRequirementPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) spill_bearing_function_count: usize,
    pub(crate) max_abstract_spill_area_bytes: u64,
    pub(crate) max_abstract_spill_area_alignment: u64,
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
    pub(crate) plan: NonAuthoritativeSpillFrameRequirementPlan,
    pub(crate) receipt: NonAuthoritativeSpillFrameRequirementReceipt,
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
