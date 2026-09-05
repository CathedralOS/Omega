use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocatorAvailabilityIdentity, GeneralizedSpillActionId, RecursiveReloadValueHomeIdentity,
    SpillPseudoInstructionId, SpillPseudoInstructionPlanIdentity, SpillPseudoOperandRewrite,
    SpillPseudoStorage, SpillPseudoStoredValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HomedSpillPseudoInstructionPlanIdentity(pub(crate) [u8; 32]);

impl HomedSpillPseudoInstructionPlanIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HomedSpillPseudoInstructionPolicy {
    RecursiveLogicalScheduleWithClosedReloadHomesV2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomedSpillPseudoInstructionPlan {
    pub spill_pseudo_instructions: SpillPseudoInstructionPlanIdentity,
    pub recursive_reload_value_homes: RecursiveReloadValueHomeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: HomedSpillPseudoInstructionPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionHomedSpillPseudoInstructions>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionHomedSpillPseudoInstructions {
    pub machine: MachineId,
    /// Required abstract spill-area extent, never a frame size.
    pub spill_area_bytes: u64,
    pub storage: Vec<SpillPseudoStorage>,
    pub instructions: Vec<HomedSpillPseudoInstruction>,
    pub rewrites: Vec<SpillPseudoOperandRewrite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomedSpillPseudoInstruction {
    Store {
        id: SpillPseudoInstructionId,
        action: GeneralizedSpillActionId,
        block: SelectedBlockId,
        point: crate::LiveRangePoint,
        before_instruction: SelectedInstructionId,
        before_reload: Option<SpillPseudoInstructionId>,
        source: SpillPseudoStoredValue,
        source_view: RegisterViewId,
        storage: GeneralizedSpillActionId,
    },
    Reload {
        id: SpillPseudoInstructionId,
        action: GeneralizedSpillActionId,
        block: SelectedBlockId,
        point: crate::LiveRangePoint,
        before_instruction: SelectedInstructionId,
        storage: GeneralizedSpillActionId,
        result: GeneralizedSpillActionId,
        destination_class: RegisterClassId,
        /// Exact target-register view proven by recursive home closure.
        destination_view: RegisterViewId,
    },
}

impl HomedSpillPseudoInstruction {
    pub const fn id(self) -> SpillPseudoInstructionId {
        match self {
            Self::Store { id, .. } | Self::Reload { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HomedSpillPseudoInstructionReceipt {
    pub(crate) identity: HomedSpillPseudoInstructionPlanIdentity,
    pub(crate) spill_pseudo_instructions: SpillPseudoInstructionPlanIdentity,
    pub(crate) recursive_reload_value_homes: RecursiveReloadValueHomeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) storage_count: usize,
    pub(crate) instruction_count: usize,
    pub(crate) reload_count: usize,
    pub(crate) rewrite_count: usize,
    pub(crate) max_spill_area_bytes: u64,
}

impl HomedSpillPseudoInstructionReceipt {
    pub const fn identity(self) -> HomedSpillPseudoInstructionPlanIdentity {
        self.identity
    }
    pub const fn spill_pseudo_instructions(self) -> SpillPseudoInstructionPlanIdentity {
        self.spill_pseudo_instructions
    }
    pub const fn recursive_reload_value_homes(self) -> RecursiveReloadValueHomeIdentity {
        self.recursive_reload_value_homes
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
    pub const fn storage_count(self) -> usize {
        self.storage_count
    }
    pub const fn instruction_count(self) -> usize {
        self.instruction_count
    }
    pub const fn reload_count(self) -> usize {
        self.reload_count
    }
    pub const fn rewrite_count(self) -> usize {
        self.rewrite_count
    }
    pub const fn max_spill_area_bytes(self) -> u64 {
        self.max_spill_area_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedHomedSpillPseudoInstructions {
    pub(crate) plan: HomedSpillPseudoInstructionPlan,
    pub(crate) receipt: HomedSpillPseudoInstructionReceipt,
}

impl ValidatedHomedSpillPseudoInstructions {
    pub const fn plan(&self) -> &HomedSpillPseudoInstructionPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> HomedSpillPseudoInstructionReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomedSpillPseudoInstructionError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    DuplicateHome {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    MissingHome {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    InvalidHome {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    InvalidPseudoOrder {
        function: usize,
    },
    WorkOverflow,
    NonCanonicalFunctions,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for HomedSpillPseudoInstructionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "homed spill-pseudo lowering failed: {self:?}")
    }
}

impl std::error::Error for HomedSpillPseudoInstructionError {}
