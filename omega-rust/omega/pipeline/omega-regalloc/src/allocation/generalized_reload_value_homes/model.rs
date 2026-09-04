use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    AbstractSpillInsertionIdentity, AllocationLegalityIdentity, AllocatorAvailabilityIdentity,
    GeneralizedSpillActionId, GeneralizedSpillActionSource, GeneralizedSpillInsertionIdentity,
    LiveRangeIdentity, LiveRangePoint, SpillRecoveryActionIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeneralizedReloadValueHomeIdentity(pub(crate) [u8; 32]);

impl GeneralizedReloadValueHomeIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeneralizedReloadValueHomePolicy {
    EpochZeroAndOneBlockLocalLowestCompatibleViewV1,
}

/// Home evidence for generalized logical reload actions. Reload action IDs are
/// compiler-private references, not selected virtual-register identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedReloadValueHomePlan {
    pub generalized_spill_insertion: GeneralizedSpillInsertionIdentity,
    pub abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub spill_recovery_actions: SpillRecoveryActionIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: GeneralizedReloadValueHomePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionGeneralizedReloadValueHomes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionGeneralizedReloadValueHomes {
    pub machine: MachineId,
    /// Canonical generalized-action order. Reanalysis stops at the first
    /// pressure outcome because later homes depend on resolving it.
    pub outcomes: Vec<GeneralizedReloadValueHomeOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneralizedReloadValueHomeOutcome {
    Assigned(GeneralizedReloadValueHomeAssignment),
    Pressure(GeneralizedReloadValuePressure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedReloadValueHomeAssignment {
    pub result: GeneralizedSpillActionId,
    pub source: GeneralizedSpillActionSource,
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub class: RegisterClassId,
    /// Complete canonical view domain across the logical reload lifetime.
    pub candidates: Vec<RegisterViewId>,
    pub view: RegisterViewId,
    pub coexisting_homes: Vec<GeneralizedReloadCoexistingHome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedReloadValuePressure {
    pub result: GeneralizedSpillActionId,
    pub source: GeneralizedSpillActionSource,
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub class: RegisterClassId,
    pub candidates: Vec<RegisterViewId>,
    /// Complete canonical occupants blocking the candidate domain at `start`.
    pub blocking_homes: Vec<GeneralizedReloadCoexistingHome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeneralizedReloadCoexistingValue {
    Original(VirtualRegisterId),
    Reload(GeneralizedSpillActionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneralizedReloadCoexistingHome {
    pub value: GeneralizedReloadCoexistingValue,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralizedReloadValueHomeReceipt {
    pub(crate) identity: GeneralizedReloadValueHomeIdentity,
    pub(crate) generalized_spill_insertion: GeneralizedSpillInsertionIdentity,
    pub(crate) abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub(crate) spill_recovery_actions: SpillRecoveryActionIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) assignment_count: usize,
    pub(crate) pressure_count: usize,
    pub(crate) retained_home_count: usize,
}

impl GeneralizedReloadValueHomeReceipt {
    pub const fn identity(self) -> GeneralizedReloadValueHomeIdentity {
        self.identity
    }
    pub const fn generalized_spill_insertion(self) -> GeneralizedSpillInsertionIdentity {
        self.generalized_spill_insertion
    }
    pub const fn abstract_spill_insertion(self) -> AbstractSpillInsertionIdentity {
        self.abstract_spill_insertion
    }
    pub const fn spill_recovery_actions(self) -> SpillRecoveryActionIdentity {
        self.spill_recovery_actions
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
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
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
    pub const fn pressure_count(self) -> usize {
        self.pressure_count
    }
    pub const fn retained_home_count(self) -> usize {
        self.retained_home_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedGeneralizedReloadValueHomes {
    pub(crate) plan: GeneralizedReloadValueHomePlan,
    pub(crate) receipt: GeneralizedReloadValueHomeReceipt,
}

impl ValidatedGeneralizedReloadValueHomes {
    pub const fn plan(&self) -> &GeneralizedReloadValueHomePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> GeneralizedReloadValueHomeReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneralizedReloadValueHomeError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    UnsupportedConstraintTopology {
        function: usize,
    },
    InvalidAction {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    MissingAction {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    NoLivePoints {
        function: usize,
        register: u32,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
    },
    NoCommonCandidate {
        function: usize,
        register: u32,
    },
    UnknownOrIncompatibleView {
        function: usize,
        view: u16,
    },
    PrefixMismatch {
        function: usize,
    },
    SecondaryPressure {
        function: usize,
        register: u32,
    },
    ReloadPressure {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    NonCanonicalAssignments {
        function: usize,
    },
    WorkOverflow,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for GeneralizedReloadValueHomeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "generalized reload-value home assignment failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralizedReloadValueHomeError {}
