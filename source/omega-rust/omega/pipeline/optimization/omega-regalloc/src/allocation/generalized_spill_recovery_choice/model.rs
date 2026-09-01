use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::SelectedBlockId;
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, GeneralizedReloadCoexistingValue,
    GeneralizedReloadValueHomeIdentity, GeneralizedSpillActionId,
    GeneralizedSpillRecoveryWorkItemId, GeneralizedSpillRecoveryWorklistIdentity, LiveRangePoint,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeneralizedSpillRecoveryChoiceIdentity(pub(crate) [u8; 32]);

impl GeneralizedSpillRecoveryChoiceIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeneralizedSpillRecoveryChoicePolicy {
    EpochTwoFarthestEndThenHighestValueV1,
}

/// Victim-choice evidence only. No field authorizes eviction or a new spill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryChoicePlan {
    pub worklist: GeneralizedSpillRecoveryWorklistIdentity,
    pub reload_value_homes: GeneralizedReloadValueHomeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: GeneralizedSpillRecoveryChoicePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub choices: Vec<GeneralizedSpillRecoveryVictimChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryVictimChoice {
    pub work_item: GeneralizedSpillRecoveryWorkItemId,
    pub function: usize,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub source_pressure: GeneralizedSpillActionId,
    pub reload_class: RegisterClassId,
    pub reload_candidates: Vec<RegisterViewId>,
    pub blocking_residents: Vec<GeneralizedSpillRecoveryResident>,
    pub contenders: Vec<GeneralizedSpillRecoveryContender>,
    pub selected_victim: GeneralizedReloadCoexistingValue,
    pub selected_victim_view: RegisterViewId,
    pub reclaimed_view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneralizedSpillRecoveryResident {
    pub value: GeneralizedReloadCoexistingValue,
    pub class: RegisterClassId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneralizedSpillRecoveryContender {
    pub value: GeneralizedReloadCoexistingValue,
    pub exclusive_end: LiveRangePoint,
    pub resident_view: RegisterViewId,
    pub reclaimed_view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryChoiceReceipt {
    pub(crate) identity: GeneralizedSpillRecoveryChoiceIdentity,
    pub(crate) worklist: GeneralizedSpillRecoveryWorklistIdentity,
    pub(crate) reload_value_homes: GeneralizedReloadValueHomeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) choice_count: usize,
    pub(crate) contender_count: usize,
}

impl GeneralizedSpillRecoveryChoiceReceipt {
    pub const fn identity(self) -> GeneralizedSpillRecoveryChoiceIdentity {
        self.identity
    }
    pub const fn worklist(self) -> GeneralizedSpillRecoveryWorklistIdentity {
        self.worklist
    }
    pub const fn reload_value_homes(self) -> GeneralizedReloadValueHomeIdentity {
        self.reload_value_homes
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
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn choice_count(self) -> usize {
        self.choice_count
    }
    pub const fn contender_count(self) -> usize {
        self.contender_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedGeneralizedSpillRecoveryChoices {
    pub(crate) plan: GeneralizedSpillRecoveryChoicePlan,
    pub(crate) receipt: GeneralizedSpillRecoveryChoiceReceipt,
}

impl ValidatedGeneralizedSpillRecoveryChoices {
    pub const fn plan(&self) -> &GeneralizedSpillRecoveryChoicePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> GeneralizedSpillRecoveryChoiceReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneralizedSpillRecoveryChoiceError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    MissingPressure {
        function: usize,
    },
    InvalidBlocker {
        function: usize,
    },
    InvalidView {
        function: usize,
        view: u16,
    },
    IntervalOverflow {
        function: usize,
    },
    NoRecoverableVictim {
        function: usize,
    },
    NonCanonicalChoices,
    WorkOverflow,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for GeneralizedSpillRecoveryChoiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "generalized spill-recovery victim choice failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralizedSpillRecoveryChoiceError {}
