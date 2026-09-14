use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocatorAvailabilityIdentity, GeneralizedReloadValueHomeIdentity, GeneralizedSpillActionId,
    GeneralizedSpillInsertionIdentity, GeneralizedSpillRecoveryChoiceIdentity,
    GeneralizedSpillRecoveryWorkItemId, LiveRangeIdentity, LiveRangePoint,
    LogicalSpillStorageClass,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeneralizedSpillRecoveryActionIdentity(pub(crate) [u8; 32]);

impl GeneralizedSpillRecoveryActionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeneralizedSpillRecoveryActionPolicy {
    EpochTwoReloadVictimLaterGeneralizedRewritesV1,
    EpochTwoOriginalVictimLaterSelectedRewritesV1,
}

/// Logical recovery obligations only. Action IDs remain compiler-private and
/// no storage row below denotes an addressable or physically placed slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryActionPlan {
    pub generalized_spill_insertion: GeneralizedSpillInsertionIdentity,
    pub reload_value_homes: GeneralizedReloadValueHomeIdentity,
    pub choices: GeneralizedSpillRecoveryChoiceIdentity,
    pub selected: Option<SelectedInstructionPlanIdentity>,
    pub ranges: Option<LiveRangeIdentity>,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: GeneralizedSpillRecoveryActionPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub actions: Vec<GeneralizedSpillRecoveryLogicalAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryLogicalAction {
    pub source_work_item: GeneralizedSpillRecoveryWorkItemId,
    pub function: usize,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub pressure_point: LiveRangePoint,
    pub source_pressure: GeneralizedSpillActionId,
    pub victim: GeneralizedSpillRecoveryVictim,
    pub victim_class: RegisterClassId,
    pub current_view: RegisterViewId,
    pub reclaimed_view: RegisterViewId,
    pub storage: GeneralizedSpillRecoveryLogicalStorage,
    pub store: GeneralizedSpillRecoveryLogicalStore,
    pub reload: GeneralizedSpillRecoveryLogicalReload,
    pub rewrites: Vec<GeneralizedSpillRecoveryLogicalUseRewrite>,
}

/// The exact compiler-private value being displaced. A selected VReg and a
/// prior logical reload remain distinct through every later boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeneralizedSpillRecoveryVictim {
    Original(VirtualRegisterId),
    Reload(GeneralizedSpillActionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryLogicalStorage {
    pub id: GeneralizedSpillActionId,
    pub class: LogicalSpillStorageClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryLogicalStore {
    /// The store must precede the existing pressured logical reload.
    pub before_pressure_reload: GeneralizedSpillActionId,
    /// Selected-program anchor only; not an inserted instruction.
    pub before_instruction: SelectedInstructionId,
    pub source: GeneralizedSpillRecoveryVictim,
    pub source_view: RegisterViewId,
    pub storage: GeneralizedSpillActionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryLogicalReload {
    /// Selected-program anchor only; not an inserted instruction.
    pub before_instruction: SelectedInstructionId,
    pub storage: GeneralizedSpillActionId,
    pub result: GeneralizedSpillActionId,
    pub destination_class: RegisterClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneralizedSpillRecoveryLogicalUseRewrite {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub result: GeneralizedSpillActionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryActionReceipt {
    pub(crate) identity: GeneralizedSpillRecoveryActionIdentity,
    pub(crate) generalized_spill_insertion: GeneralizedSpillInsertionIdentity,
    pub(crate) reload_value_homes: GeneralizedReloadValueHomeIdentity,
    pub(crate) choices: GeneralizedSpillRecoveryChoiceIdentity,
    pub(crate) selected: Option<SelectedInstructionPlanIdentity>,
    pub(crate) ranges: Option<LiveRangeIdentity>,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) action_count: usize,
    pub(crate) rewrite_count: usize,
}

impl GeneralizedSpillRecoveryActionReceipt {
    pub const fn identity(self) -> GeneralizedSpillRecoveryActionIdentity {
        self.identity
    }
    pub const fn generalized_spill_insertion(self) -> GeneralizedSpillInsertionIdentity {
        self.generalized_spill_insertion
    }
    pub const fn reload_value_homes(self) -> GeneralizedReloadValueHomeIdentity {
        self.reload_value_homes
    }
    pub const fn choices(self) -> GeneralizedSpillRecoveryChoiceIdentity {
        self.choices
    }
    pub const fn selected(self) -> Option<SelectedInstructionPlanIdentity> {
        self.selected
    }
    pub const fn ranges(self) -> Option<LiveRangeIdentity> {
        self.ranges
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
    pub const fn action_count(self) -> usize {
        self.action_count
    }
    pub const fn rewrite_count(self) -> usize {
        self.rewrite_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedGeneralizedSpillRecoveryActions {
    pub(crate) plan: GeneralizedSpillRecoveryActionPlan,
    pub(crate) receipt: GeneralizedSpillRecoveryActionReceipt,
}

impl ValidatedGeneralizedSpillRecoveryActions {
    pub const fn plan(&self) -> &GeneralizedSpillRecoveryActionPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> GeneralizedSpillRecoveryActionReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneralizedSpillRecoveryActionError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    UnsupportedVictim {
        function: usize,
    },
    MissingOriginalVictim {
        function: usize,
        register: VirtualRegisterId,
    },
    NoFutureOriginalRewrite {
        function: usize,
        register: VirtualRegisterId,
    },
    InvalidOriginalRewrite {
        function: usize,
        register: VirtualRegisterId,
    },
    MissingVictimAction {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    MissingPressureReload {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    NoFutureRewrite {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    InvalidRewrite {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    NonCanonicalActions,
    WorkOverflow,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for GeneralizedSpillRecoveryActionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "generalized spill-recovery action planning failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralizedSpillRecoveryActionError {}
