use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::SelectedBlockId;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocatorAvailabilityIdentity, LiveRangePoint, LogicalSpillOperationIdentity,
    LogicalSpillStorageClass, LogicalSpillStorageId,
};

/// Identity of a canonical, independently replayable stack-slot coloring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StackSlotColoringIdentity(pub(crate) [u8; 32]);

impl StackSlotColoringIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Closed target-neutral policy. Lifetimes are closed, so touching endpoints conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StackSlotColoringPolicy {
    BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
}

/// Target-neutral assignments relative to the beginning of an unspecified spill area.
///
/// This artifact grants no final frame, stack-pointer offset, instruction,
/// unwind, ABI-layout, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackSlotColoringPlan {
    pub logical_spill_operations: LogicalSpillOperationIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: StackSlotColoringPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionStackSlotColoring>,
}

impl StackSlotColoringPlan {
    /// Canonical transport only. Independent replay is still required for authority.
    pub fn encode(&self) -> Vec<u8> {
        super::codec::encode(self)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, StackSlotColoringDecodeError> {
        super::codec::decode(encoded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionStackSlotColoring {
    pub machine: MachineId,
    pub assignments: Vec<StackSlotAssignment>,
    /// Bytes required from a future spill area. This is not a frame size.
    pub spill_area_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackSlotAssignment {
    pub storage: LogicalSpillStorageId,
    pub class: LogicalSpillStorageClass,
    pub block: SelectedBlockId,
    pub live_from: LiveRangePoint,
    pub live_through: LiveRangePoint,
    pub size_bytes: u64,
    pub alignment_bytes: u64,
    /// Byte offset relative to the beginning of an as-yet-unlaid-out spill area.
    pub spill_area_offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackSlotColoringValidationReceipt {
    pub(crate) identity: StackSlotColoringIdentity,
    pub(crate) logical_spill_operations: LogicalSpillOperationIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) policy: StackSlotColoringPolicy,
    pub(crate) budget: OptimizationWorkBudget,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) assignment_count: usize,
    pub(crate) distinct_slot_count: usize,
    pub(crate) reused_assignment_count: usize,
    pub(crate) max_function_spill_area_bytes: u64,
}

impl StackSlotColoringValidationReceipt {
    pub const fn identity(self) -> StackSlotColoringIdentity {
        self.identity
    }
    pub const fn logical_spill_operations(self) -> LogicalSpillOperationIdentity {
        self.logical_spill_operations
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
    pub const fn policy(self) -> StackSlotColoringPolicy {
        self.policy
    }
    pub const fn budget(self) -> OptimizationWorkBudget {
        self.budget
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
    pub const fn distinct_slot_count(self) -> usize {
        self.distinct_slot_count
    }
    pub const fn reused_assignment_count(self) -> usize {
        self.reused_assignment_count
    }
    pub const fn max_function_spill_area_bytes(self) -> u64 {
        self.max_function_spill_area_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedStackSlotColoring {
    pub(crate) plan: StackSlotColoringPlan,
    pub(crate) receipt: StackSlotColoringValidationReceipt,
}

impl ValidatedStackSlotColoring {
    pub const fn plan(&self) -> &StackSlotColoringPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> StackSlotColoringValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackSlotColoringError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedStorageClass {
        function: usize,
        storage: LogicalSpillStorageId,
    },
    InvalidLogicalAction {
        function: usize,
        storage: LogicalSpillStorageId,
    },
    InvalidInterval {
        function: usize,
        storage: LogicalSpillStorageId,
    },
    DuplicateStorage {
        function: usize,
        storage: LogicalSpillStorageId,
    },
    FunctionMismatch {
        function: usize,
    },
    WorkOverflow,
    OffsetOverflow {
        function: usize,
    },
    NonCanonicalAssignments {
        function: usize,
    },
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for StackSlotColoringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "stack-slot coloring failed: {self:?}")
    }
}

impl std::error::Error for StackSlotColoringError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackSlotColoringDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownStorageClass(u8),
    InvalidMachineId(u64),
    InvalidBudget,
    InvalidUsage,
    InvalidFuelSchedule(u32),
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for StackSlotColoringDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid stack-slot-coloring encoding: {self:?}")
    }
}

impl std::error::Error for StackSlotColoringDecodeError {}
