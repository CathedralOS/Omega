use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{SelectedInstructionId, VirtualRegisterId};
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocatorAvailabilityIdentity, LiveRangePoint, LogicalReloadValueId,
    LogicalSpillOperationIdentity, LogicalSpillStorageClass, LogicalSpillStorageId,
    LogicalSpillUseRewrite, StackSlotColoringIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbstractSpillInsertionIdentity(pub(crate) [u8; 32]);

impl AbstractSpillInsertionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbstractSpillInsertionPolicy {
    BlockLocalNonAddressUnsignedU64AbstractSpillAreaV1,
}

/// Exact symbolic insertion schedule relative to an abstract spill-area
/// origin. It is not a selected plan, post-allocation machine plan, frame, or
/// emission artifact and grants none of those authorities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractSpillInsertionPlan {
    pub logical_spill_operations: LogicalSpillOperationIdentity,
    pub stack_slot_coloring: StackSlotColoringIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: AbstractSpillInsertionPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionAbstractSpillInsertion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAbstractSpillInsertion {
    pub machine: MachineId,
    /// Required bytes in an abstract spill area; this is not a frame size.
    pub spill_area_bytes: u64,
    pub action: Option<AbstractSpillInsertionAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractSpillInsertionAction {
    pub pressure_point: LiveRangePoint,
    pub incoming: VirtualRegisterId,
    /// Physical view recovered for the incoming definition by the spill.
    pub incoming_view: RegisterViewId,
    pub victim: VirtualRegisterId,
    pub victim_view: RegisterViewId,
    pub slot: AbstractSpillAreaSlot,
    pub store: AbstractSpillAreaStore,
    pub reload: AbstractSpillAreaReload,
    pub rewrites: Vec<LogicalSpillUseRewrite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractSpillAreaSlot {
    pub storage: LogicalSpillStorageId,
    pub class: LogicalSpillStorageClass,
    pub size_bytes: u64,
    pub alignment_bytes: u64,
    /// Relative to an unspecified spill-area origin, never SP or FP.
    pub spill_area_offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractSpillAreaStore {
    pub before_instruction: SelectedInstructionId,
    pub source: VirtualRegisterId,
    pub source_view: RegisterViewId,
    pub slot: LogicalSpillStorageId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractSpillAreaReload {
    pub before_instruction: SelectedInstructionId,
    pub slot: LogicalSpillStorageId,
    pub result: LogicalReloadValueId,
    /// The future post-insertion allocator still owns the physical home.
    pub destination_class: RegisterClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractSpillInsertionReceipt {
    pub(crate) identity: AbstractSpillInsertionIdentity,
    pub(crate) logical_spill_operations: LogicalSpillOperationIdentity,
    pub(crate) stack_slot_coloring: StackSlotColoringIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) action_count: usize,
    pub(crate) access_count: usize,
    pub(crate) rewritten_use_count: usize,
    pub(crate) max_spill_area_bytes: u64,
}

impl AbstractSpillInsertionReceipt {
    pub const fn identity(self) -> AbstractSpillInsertionIdentity {
        self.identity
    }
    pub const fn logical_spill_operations(self) -> LogicalSpillOperationIdentity {
        self.logical_spill_operations
    }
    pub const fn stack_slot_coloring(self) -> StackSlotColoringIdentity {
        self.stack_slot_coloring
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
    pub const fn access_count(self) -> usize {
        self.access_count
    }
    pub const fn rewritten_use_count(self) -> usize {
        self.rewritten_use_count
    }
    pub const fn max_spill_area_bytes(self) -> u64 {
        self.max_spill_area_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAbstractSpillInsertion {
    pub(crate) plan: AbstractSpillInsertionPlan,
    pub(crate) receipt: AbstractSpillInsertionReceipt,
}

impl ValidatedAbstractSpillInsertion {
    pub const fn plan(&self) -> &AbstractSpillInsertionPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> AbstractSpillInsertionReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractSpillInsertionError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    MissingSlot {
        function: usize,
        storage: LogicalSpillStorageId,
    },
    NonCanonicalSchedule {
        function: usize,
    },
    WorkOverflow,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for AbstractSpillInsertionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "abstract spill insertion failed: {self:?}")
    }
}

impl std::error::Error for AbstractSpillInsertionError {}
