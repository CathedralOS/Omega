use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId};
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::{
    AllocatorAvailabilityIdentity, GeneralizedSpillActionId,
    HomedSpillPseudoInstructionPlanIdentity, LiveRangePoint, LogicalSpillStorageClass,
    SpillPseudoInstructionId, SpillPseudoStoredValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbstractSpillMemoryEffectPlanIdentity(pub(crate) [u8; 32]);

impl AbstractSpillMemoryEffectPlanIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbstractSpillMemoryEffectPolicy {
    HomedPseudoReadWriteV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractSpillMemoryEffectPlan {
    pub homed_spill_pseudo_instructions: HomedSpillPseudoInstructionPlanIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: AbstractSpillMemoryEffectPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionAbstractSpillMemoryEffects>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAbstractSpillMemoryEffects {
    pub machine: MachineId,
    /// Required abstract extent, never an allocated frame size.
    pub spill_area_bytes: u64,
    pub effects: Vec<AbstractSpillMemoryEffect>,
}

/// A target-neutral access obligation over an abstract spill-area slice.
/// Neither variant is an executable load/store or a fault claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractSpillMemoryEffect {
    Write {
        pseudo: SpillPseudoInstructionId,
        action: GeneralizedSpillActionId,
        block: SelectedBlockId,
        point: LiveRangePoint,
        before_instruction: SelectedInstructionId,
        before_reload: Option<SpillPseudoInstructionId>,
        source: SpillPseudoStoredValue,
        source_view: RegisterViewId,
        storage: GeneralizedSpillActionId,
        storage_class: LogicalSpillStorageClass,
        spill_area_offset: u64,
        size_bytes: u64,
        alignment_bytes: u64,
    },
    Read {
        pseudo: SpillPseudoInstructionId,
        action: GeneralizedSpillActionId,
        block: SelectedBlockId,
        point: LiveRangePoint,
        before_instruction: SelectedInstructionId,
        storage: GeneralizedSpillActionId,
        storage_class: LogicalSpillStorageClass,
        spill_area_offset: u64,
        size_bytes: u64,
        alignment_bytes: u64,
        result: GeneralizedSpillActionId,
        destination_class: RegisterClassId,
        destination_view: RegisterViewId,
    },
}

impl AbstractSpillMemoryEffect {
    pub const fn pseudo(self) -> SpillPseudoInstructionId {
        match self {
            Self::Write { pseudo, .. } | Self::Read { pseudo, .. } => pseudo,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractSpillMemoryEffectReceipt {
    pub(crate) identity: AbstractSpillMemoryEffectPlanIdentity,
    pub(crate) homed_spill_pseudo_instructions: HomedSpillPseudoInstructionPlanIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) read_count: usize,
    pub(crate) write_count: usize,
    pub(crate) max_spill_area_bytes: u64,
}

impl AbstractSpillMemoryEffectReceipt {
    pub const fn identity(self) -> AbstractSpillMemoryEffectPlanIdentity {
        self.identity
    }
    pub const fn homed_spill_pseudo_instructions(self) -> HomedSpillPseudoInstructionPlanIdentity {
        self.homed_spill_pseudo_instructions
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
    pub const fn read_count(self) -> usize {
        self.read_count
    }
    pub const fn write_count(self) -> usize {
        self.write_count
    }
    pub const fn max_spill_area_bytes(self) -> u64 {
        self.max_spill_area_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAbstractSpillMemoryEffects {
    pub(crate) plan: AbstractSpillMemoryEffectPlan,
    pub(crate) receipt: AbstractSpillMemoryEffectReceipt,
}

impl ValidatedAbstractSpillMemoryEffects {
    pub const fn plan(&self) -> &AbstractSpillMemoryEffectPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> AbstractSpillMemoryEffectReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractSpillMemoryEffectError {
    RootMismatch,
    UnsupportedPolicy,
    DuplicateStorage {
        function: usize,
        storage: GeneralizedSpillActionId,
    },
    MissingStorage {
        function: usize,
        storage: GeneralizedSpillActionId,
    },
    InvalidStorage {
        function: usize,
        storage: GeneralizedSpillActionId,
    },
    InvalidEffectOrder {
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

impl std::fmt::Display for AbstractSpillMemoryEffectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "abstract spill memory-effect derivation failed: {self:?}"
        )
    }
}

impl std::error::Error for AbstractSpillMemoryEffectError {}
