use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use psi_core::{FuelScheduleIdentity, MachineId, ScalarType};

use crate::{
    AbstractSpillInsertionIdentity, AllocationLegalityIdentity, AllocatorAvailabilityIdentity,
    LiveRangeIdentity, LiveRangePoint, LogicalReloadValueId, LogicalSpillStorageClass,
    SpillRecoveryChoiceIdentity, SpillRecoveryWorklistIdentity, SyntheticReloadValueId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpillRecoveryActionIdentity(pub(crate) [u8; 32]);

impl SpillRecoveryActionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpillRecoveryActionPolicy {
    EpochOneActiveResidentInstructionResultU64LaterFlexibleUsesV1,
}

/// Logical recovery obligations only. None of these rows is a selected
/// instruction, physical slot, frame address, or memory-effect declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillRecoveryActionPlan {
    pub selected: SelectedInstructionPlanIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub worklist: SpillRecoveryWorklistIdentity,
    pub choices: SpillRecoveryChoiceIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: SpillRecoveryActionPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub actions: Vec<SpillRecoveryLogicalAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpillRecoveryLogicalStorageId {
    pub epoch: u32,
    pub ordinal: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpillRecoveryLogicalReloadId {
    pub epoch: u32,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillRecoveryLogicalAction {
    pub source_work_item: SyntheticReloadValueId,
    pub function: usize,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub pressure_point: LiveRangePoint,
    pub source_reload: LogicalReloadValueId,
    pub incoming_class: RegisterClassId,
    pub victim: VirtualRegisterId,
    pub victim_class: RegisterClassId,
    pub victim_scalar_type: ScalarType,
    pub victim_origin: VirtualRegisterOrigin,
    pub victim_definition_site: ValueDefinitionSite,
    pub current_view: RegisterViewId,
    pub reclaimed_view: RegisterViewId,
    pub storage: SpillRecoveryLogicalStorage,
    pub store: SpillRecoveryLogicalStore,
    pub reload: SpillRecoveryLogicalReload,
    pub rewrites: Vec<SpillRecoveryLogicalUseRewrite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillRecoveryLogicalStorage {
    pub id: SpillRecoveryLogicalStorageId,
    pub class: LogicalSpillStorageClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillRecoveryLogicalStore {
    /// The store must precede this already-logical source reload.
    pub before_source_reload: LogicalReloadValueId,
    /// Selected-program anchor only; this is not an inserted instruction.
    pub before_instruction: SelectedInstructionId,
    pub source: VirtualRegisterId,
    pub storage: SpillRecoveryLogicalStorageId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillRecoveryLogicalReload {
    /// Selected-program anchor only; this is not an inserted instruction.
    pub before_instruction: SelectedInstructionId,
    pub storage: SpillRecoveryLogicalStorageId,
    pub result: SpillRecoveryLogicalReloadId,
    pub destination_class: RegisterClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpillRecoveryLogicalUseRewrite {
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub result: SpillRecoveryLogicalReloadId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillRecoveryActionReceipt {
    pub(crate) identity: SpillRecoveryActionIdentity,
    pub(crate) choices: SpillRecoveryChoiceIdentity,
    pub(crate) worklist: SpillRecoveryWorklistIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) action_count: usize,
    pub(crate) rewrite_count: usize,
}

impl SpillRecoveryActionReceipt {
    pub const fn identity(self) -> SpillRecoveryActionIdentity {
        self.identity
    }
    pub const fn choices(self) -> SpillRecoveryChoiceIdentity {
        self.choices
    }
    pub const fn worklist(self) -> SpillRecoveryWorklistIdentity {
        self.worklist
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
pub struct ValidatedSpillRecoveryActions {
    pub(crate) plan: SpillRecoveryActionPlan,
    pub(crate) receipt: SpillRecoveryActionReceipt,
}

impl ValidatedSpillRecoveryActions {
    pub const fn plan(&self) -> &SpillRecoveryActionPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> SpillRecoveryActionReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpillRecoveryActionError {
    RootMismatch,
    UnsupportedPolicy,
    SourceWorkItemMismatch,
    FunctionMismatch {
        function: usize,
    },
    UnsupportedVictimRole {
        function: usize,
        register: u32,
    },
    UnsupportedScalarType {
        function: usize,
        register: u32,
    },
    UnsupportedOrigin {
        function: usize,
        register: u32,
    },
    UnsupportedRangeShape {
        function: usize,
        register: u32,
    },
    VictimUsedAtPressure {
        function: usize,
        register: u32,
    },
    FutureFixedUse {
        function: usize,
        register: u32,
    },
    NoFutureUse {
        function: usize,
        register: u32,
    },
    FutureUseMismatch {
        function: usize,
        register: u32,
    },
    NonCanonicalNamespace,
    NonCanonicalActions,
    WorkOverflow,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for SpillRecoveryActionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "logical spill-recovery action failed: {self:?}")
    }
}

impl std::error::Error for SpillRecoveryActionError {}
