use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, GeneralizedReloadValueHomeIdentity,
    GeneralizedSpillActionId, GeneralizedSpillRecoveryActionIdentity, LiveRangeIdentity,
    LiveRangePoint, RecursiveSpillActionSource, RecursiveSpillInsertionIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecursiveReloadValueHomeIdentity(pub(crate) [u8; 32]);

impl RecursiveReloadValueHomeIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecursiveReloadValueHomePolicy {
    CompleteBlockLocalLowestCompatibleViewV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveReloadValueHomePlan {
    pub recursive_spill_insertion: RecursiveSpillInsertionIdentity,
    pub recovery_actions: GeneralizedSpillRecoveryActionIdentity,
    pub prior_reload_value_homes: GeneralizedReloadValueHomeIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: RecursiveReloadValueHomePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionRecursiveReloadValueHomes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRecursiveReloadValueHomes {
    pub machine: MachineId,
    /// Canonical logical-action order. Every recursive reload has one row.
    pub assignments: Vec<RecursiveReloadValueHomeAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveReloadValueHomeAssignment {
    pub result: GeneralizedSpillActionId,
    pub source: RecursiveSpillActionSource,
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub class: RegisterClassId,
    /// Complete canonical view domain across this post-store reload segment.
    pub candidates: Vec<RegisterViewId>,
    pub view: RegisterViewId,
    /// Complete canonical roster of values coexisting anywhere in the segment.
    pub coexisting_homes: Vec<RecursiveReloadCoexistingHome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecursiveReloadCoexistingValue {
    Original(VirtualRegisterId),
    Reload(GeneralizedSpillActionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecursiveReloadCoexistingHome {
    pub value: RecursiveReloadCoexistingValue,
    pub class: RegisterClassId,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecursiveReloadValueHomeReceipt {
    pub(crate) identity: RecursiveReloadValueHomeIdentity,
    pub(crate) recursive_spill_insertion: RecursiveSpillInsertionIdentity,
    pub(crate) recovery_actions: GeneralizedSpillRecoveryActionIdentity,
    pub(crate) prior_reload_value_homes: GeneralizedReloadValueHomeIdentity,
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
    pub(crate) retained_home_count: usize,
}

impl RecursiveReloadValueHomeReceipt {
    pub const fn identity(self) -> RecursiveReloadValueHomeIdentity {
        self.identity
    }
    pub const fn recursive_spill_insertion(self) -> RecursiveSpillInsertionIdentity {
        self.recursive_spill_insertion
    }
    pub const fn recovery_actions(self) -> GeneralizedSpillRecoveryActionIdentity {
        self.recovery_actions
    }
    pub const fn prior_reload_value_homes(self) -> GeneralizedReloadValueHomeIdentity {
        self.prior_reload_value_homes
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
    pub const fn retained_home_count(self) -> usize {
        self.retained_home_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRecursiveReloadValueHomes {
    pub(crate) plan: RecursiveReloadValueHomePlan,
    pub(crate) receipt: RecursiveReloadValueHomeReceipt,
}

impl ValidatedRecursiveReloadValueHomes {
    pub const fn plan(&self) -> &RecursiveReloadValueHomePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> RecursiveReloadValueHomeReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecursiveReloadValueHomeError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    UnsupportedConstraintTopology {
        function: usize,
    },
    MissingPressure {
        function: usize,
    },
    MultiplePressures {
        function: usize,
    },
    InvalidPriorOutcome {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    InvalidRecursiveAction {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    MissingSourceRegister {
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
    VictimMismatch {
        function: usize,
        action: GeneralizedSpillActionId,
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

impl std::fmt::Display for RecursiveReloadValueHomeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "recursive reload-value home closure failed: {self:?}"
        )
    }
}

impl std::error::Error for RecursiveReloadValueHomeError {}
