use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocatorAvailabilityIdentity, GeneralizedSpillActionId, LiveRangePoint,
    LogicalSpillStorageClass, RecursiveSpillInsertionIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpillPseudoInstructionPlanIdentity(pub(crate) [u8; 32]);

impl SpillPseudoInstructionPlanIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpillPseudoInstructionPolicy {
    RecursiveLogicalScheduleV1,
}

/// A function-local compiler-private pseudo identity. It is not a selected or
/// machine instruction identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpillPseudoInstructionId {
    pub ordinal: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpillPseudoStoredValue {
    Original(VirtualRegisterId),
    Reload {
        action: GeneralizedSpillActionId,
        producer: SpillPseudoInstructionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillPseudoInstructionPlan {
    pub recursive_spill_insertion: RecursiveSpillInsertionIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: SpillPseudoInstructionPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionSpillPseudoInstructions>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSpillPseudoInstructions {
    pub machine: MachineId,
    /// Required abstract spill-area extent, never a frame size.
    pub spill_area_bytes: u64,
    pub storage: Vec<SpillPseudoStorage>,
    pub instructions: Vec<SpillPseudoInstruction>,
    pub rewrites: Vec<SpillPseudoOperandRewrite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillPseudoStorage {
    pub id: GeneralizedSpillActionId,
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
pub enum SpillPseudoInstruction {
    Store {
        id: SpillPseudoInstructionId,
        action: GeneralizedSpillActionId,
        block: SelectedBlockId,
        point: LiveRangePoint,
        before_instruction: SelectedInstructionId,
        /// Compiler-private reload pseudo that this store must precede.
        before_reload: Option<SpillPseudoInstructionId>,
        source: SpillPseudoStoredValue,
        source_view: RegisterViewId,
        storage: GeneralizedSpillActionId,
    },
    Reload {
        id: SpillPseudoInstructionId,
        action: GeneralizedSpillActionId,
        block: SelectedBlockId,
        point: LiveRangePoint,
        before_instruction: SelectedInstructionId,
        storage: GeneralizedSpillActionId,
        result: GeneralizedSpillActionId,
        destination_class: RegisterClassId,
    },
}

impl SpillPseudoInstruction {
    pub const fn id(self) -> SpillPseudoInstructionId {
        match self {
            Self::Store { id, .. } | Self::Reload { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillPseudoOperandRewrite {
    pub action: GeneralizedSpillActionId,
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub result: GeneralizedSpillActionId,
    pub producer: SpillPseudoInstructionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillPseudoInstructionReceipt {
    pub(crate) identity: SpillPseudoInstructionPlanIdentity,
    pub(crate) recursive_spill_insertion: RecursiveSpillInsertionIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) storage_count: usize,
    pub(crate) instruction_count: usize,
    pub(crate) rewrite_count: usize,
    pub(crate) max_spill_area_bytes: u64,
}

impl SpillPseudoInstructionReceipt {
    pub const fn identity(self) -> SpillPseudoInstructionPlanIdentity {
        self.identity
    }
    pub const fn recursive_spill_insertion(self) -> RecursiveSpillInsertionIdentity {
        self.recursive_spill_insertion
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
    pub const fn rewrite_count(self) -> usize {
        self.rewrite_count
    }
    pub const fn max_spill_area_bytes(self) -> u64 {
        self.max_spill_area_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSpillPseudoInstructions {
    pub(crate) plan: SpillPseudoInstructionPlan,
    pub(crate) receipt: SpillPseudoInstructionReceipt,
}

impl ValidatedSpillPseudoInstructions {
    pub const fn plan(&self) -> &SpillPseudoInstructionPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> SpillPseudoInstructionReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpillPseudoInstructionError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionMismatch {
        function: usize,
    },
    DuplicateStorage {
        function: usize,
        storage: GeneralizedSpillActionId,
    },
    MissingStorage {
        function: usize,
        storage: GeneralizedSpillActionId,
    },
    DuplicateReload {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    MissingReload {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    InvalidPseudoOrder {
        function: usize,
    },
    InvalidRewrite {
        function: usize,
        action: GeneralizedSpillActionId,
    },
    IdentityOverflow,
    WorkOverflow,
    NonCanonicalFunctions,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for SpillPseudoInstructionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "spill-pseudo lowering failed: {self:?}")
    }
}

impl std::error::Error for SpillPseudoInstructionError {}
