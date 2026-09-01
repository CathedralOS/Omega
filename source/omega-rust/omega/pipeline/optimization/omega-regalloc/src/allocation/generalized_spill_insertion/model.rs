use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    AbstractSpillInsertionIdentity, LiveRangePoint, LogicalReloadValueId, LogicalSpillStorageClass,
    LogicalSpillStorageId, SpillRecoveryActionIdentity, SpillRecoveryLogicalReloadId,
    SpillRecoveryLogicalStorageId, SyntheticReloadValueId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeneralizedSpillInsertionIdentity(pub(crate) [u8; 32]);

impl GeneralizedSpillInsertionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeneralizedSpillInsertionPolicy {
    EpochZeroAndOneBlockLocalUnsignedU64ClosedIntervalFirstFitV1,
}

/// A compiler-private action namespace. It is not a selected virtual register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeneralizedSpillActionId {
    pub epoch: u32,
    pub ordinal: u32,
}

/// The independently validated logical row from which a generalized action came.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneralizedSpillActionSource {
    EpochZero {
        storage: LogicalSpillStorageId,
        reload: LogicalReloadValueId,
    },
    EpochOne {
        work_item: SyntheticReloadValueId,
        storage: SpillRecoveryLogicalStorageId,
        source_reload: LogicalReloadValueId,
        reload: SpillRecoveryLogicalReloadId,
    },
}

/// Target-neutral slot lifetimes and insertion events relative to an abstract
/// spill-area origin. No row is a selected or machine instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedSpillInsertionPlan {
    pub abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub spill_recovery_actions: SpillRecoveryActionIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: crate::AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: GeneralizedSpillInsertionPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionGeneralizedSpillInsertion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionGeneralizedSpillInsertion {
    pub machine: MachineId,
    /// Bytes required from a future spill area. This is not a frame size.
    pub spill_area_bytes: u64,
    pub slots: Vec<GeneralizedSpillSlot>,
    /// Canonical point order. Stores precede reloads at an equal point and
    /// reloads precede rewrites at an equal point.
    pub schedule: Vec<GeneralizedSpillEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralizedSpillSlot {
    pub action: GeneralizedSpillActionId,
    pub source: GeneralizedSpillActionSource,
    pub class: LogicalSpillStorageClass,
    pub block: SelectedBlockId,
    pub live_from: LiveRangePoint,
    pub live_through: LiveRangePoint,
    pub size_bytes: u64,
    pub alignment_bytes: u64,
    /// Relative to an unspecified spill-area origin, never SP or FP.
    pub spill_area_offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneralizedSpillEvent {
    Store {
        action: GeneralizedSpillActionId,
        point: LiveRangePoint,
        before_instruction: SelectedInstructionId,
        /// Epoch-one stores must precede the epoch-zero reload that triggered
        /// their recovery item. Epoch-zero stores have no logical dependency.
        before_reload: Option<GeneralizedSpillActionId>,
        source: VirtualRegisterId,
        source_view: RegisterViewId,
        slot: GeneralizedSpillActionId,
    },
    Reload {
        action: GeneralizedSpillActionId,
        point: LiveRangePoint,
        before_instruction: SelectedInstructionId,
        slot: GeneralizedSpillActionId,
        result: GeneralizedSpillActionId,
        destination_class: RegisterClassId,
    },
    Rewrite {
        action: GeneralizedSpillActionId,
        block: SelectedBlockId,
        point: LiveRangePoint,
        instruction: SelectedInstructionId,
        operand: u16,
        result: GeneralizedSpillActionId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralizedSpillInsertionReceipt {
    pub(crate) identity: GeneralizedSpillInsertionIdentity,
    pub(crate) abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub(crate) spill_recovery_actions: SpillRecoveryActionIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: crate::AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) action_count: usize,
    pub(crate) event_count: usize,
    pub(crate) max_spill_area_bytes: u64,
}

impl GeneralizedSpillInsertionReceipt {
    pub const fn identity(self) -> GeneralizedSpillInsertionIdentity {
        self.identity
    }
    pub const fn abstract_spill_insertion(self) -> AbstractSpillInsertionIdentity {
        self.abstract_spill_insertion
    }
    pub const fn spill_recovery_actions(self) -> SpillRecoveryActionIdentity {
        self.spill_recovery_actions
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> crate::AllocatorAvailabilityIdentity {
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
    pub const fn action_count(self) -> usize {
        self.action_count
    }
    pub const fn event_count(self) -> usize {
        self.event_count
    }
    pub const fn max_spill_area_bytes(self) -> u64 {
        self.max_spill_area_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedGeneralizedSpillInsertion {
    pub(crate) plan: GeneralizedSpillInsertionPlan,
    pub(crate) receipt: GeneralizedSpillInsertionReceipt,
}

impl ValidatedGeneralizedSpillInsertion {
    pub const fn plan(&self) -> &GeneralizedSpillInsertionPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> GeneralizedSpillInsertionReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneralizedSpillInsertionError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    MissingEpochZeroAction {
        function: usize,
    },
    MissingSourceReload {
        function: usize,
        reload: LogicalReloadValueId,
    },
    InvalidEpochZeroAction {
        function: usize,
    },
    InvalidEpochOneAction {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    UnsupportedStorageClass {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    DuplicateAction {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    InvalidLifetime {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    NamespaceOverflow,
    OffsetOverflow {
        function: usize,
    },
    WorkOverflow,
    NonCanonicalSlots {
        function: usize,
    },
    NonCanonicalSchedule {
        function: usize,
    },
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for GeneralizedSpillInsertionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "generalized abstract spill insertion failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralizedSpillInsertionError {}
