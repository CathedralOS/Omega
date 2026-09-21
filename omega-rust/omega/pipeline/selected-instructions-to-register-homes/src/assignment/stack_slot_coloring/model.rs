use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::TargetRegisterEnvironmentIdentity;
use semantic_vocabulary::FuelScheduleIdentity;

use crate::{
    AllocatorAvailabilityIdentity, LogicalSpillOperationIdentity, LogicalSpillStorageId,
    StackSlotColoringIdentity, StackSlotColoringPlan, StackSlotColoringPolicy,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackSlotColoringValidationReceipt {
    pub(crate) identity: StackSlotColoringIdentity,
    pub(crate) logical_spill_operations: LogicalSpillOperationIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: optimization_core::OptimizationUnitIdentity,
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
    pub const fn optimization_unit(self) -> optimization_core::OptimizationUnitIdentity {
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
