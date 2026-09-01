use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocatorAvailabilityIdentity, GeneralizedSpillActionId, GeneralizedSpillActionSource,
    GeneralizedSpillInsertionIdentity, GeneralizedSpillRecoveryActionIdentity,
    GeneralizedSpillRecoveryWorkItemId, LiveRangePoint, LogicalSpillStorageClass,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecursiveSpillInsertionIdentity(pub(crate) [u8; 32]);

impl RecursiveSpillInsertionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecursiveSpillInsertionPolicy {
    EpochTwoReloadVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecursiveSpillActionSource {
    Prior(GeneralizedSpillActionSource),
    EpochTwo {
        work_item: GeneralizedSpillRecoveryWorkItemId,
        source_pressure: GeneralizedSpillActionId,
        victim: GeneralizedSpillActionId,
    },
}

/// The value logically stored. Reload actions never masquerade as source vregs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecursiveSpillStoredValue {
    Original(VirtualRegisterId),
    Reload(GeneralizedSpillActionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveSpillInsertionPlan {
    pub generalized_spill_insertion: GeneralizedSpillInsertionIdentity,
    pub recovery_actions: GeneralizedSpillRecoveryActionIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: RecursiveSpillInsertionPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionRecursiveSpillInsertion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRecursiveSpillInsertion {
    pub machine: MachineId,
    /// Bytes required from a future spill area; never a frame size.
    pub spill_area_bytes: u64,
    pub slots: Vec<RecursiveSpillSlot>,
    pub schedule: Vec<RecursiveSpillEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecursiveSpillSlot {
    pub action: GeneralizedSpillActionId,
    pub source: RecursiveSpillActionSource,
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
pub enum RecursiveSpillEvent {
    Store {
        action: GeneralizedSpillActionId,
        point: LiveRangePoint,
        before_instruction: SelectedInstructionId,
        before_reload: Option<GeneralizedSpillActionId>,
        source: RecursiveSpillStoredValue,
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
pub struct RecursiveSpillInsertionReceipt {
    pub(crate) identity: RecursiveSpillInsertionIdentity,
    pub(crate) generalized_spill_insertion: GeneralizedSpillInsertionIdentity,
    pub(crate) recovery_actions: GeneralizedSpillRecoveryActionIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) action_count: usize,
    pub(crate) event_count: usize,
    pub(crate) max_spill_area_bytes: u64,
}

impl RecursiveSpillInsertionReceipt {
    pub const fn identity(self) -> RecursiveSpillInsertionIdentity {
        self.identity
    }
    pub const fn generalized_spill_insertion(self) -> GeneralizedSpillInsertionIdentity {
        self.generalized_spill_insertion
    }
    pub const fn recovery_actions(self) -> GeneralizedSpillRecoveryActionIdentity {
        self.recovery_actions
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
pub struct ValidatedRecursiveSpillInsertion {
    pub(crate) plan: RecursiveSpillInsertionPlan,
    pub(crate) receipt: RecursiveSpillInsertionReceipt,
}

impl ValidatedRecursiveSpillInsertion {
    pub const fn plan(&self) -> &RecursiveSpillInsertionPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> RecursiveSpillInsertionReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecursiveSpillInsertionError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    MissingBaseAction {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    InvalidRecoveryAction {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    UnsupportedRecoveryVictim {
        function: usize,
        action: GeneralizedSpillActionId,
        victim: crate::GeneralizedSpillRecoveryVictim,
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

impl std::fmt::Display for RecursiveSpillInsertionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "recursive abstract spill insertion failed: {self:?}"
        )
    }
}

impl std::error::Error for RecursiveSpillInsertionError {}
