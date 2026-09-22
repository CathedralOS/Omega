//! Optimizer module role: executable entrance. V2 homed spill-pseudo lowering.
//!
//! This join enriches validated V1 compiler-private pseudos with the exact
//! destination view from final recursive reload-home closure. It creates no
//! selected or machine instruction, address, memory effect, frame, trap,
//! encoding, emission, or publication authority.

mod compute;
mod identity;
mod replay;
mod validate;

use crate::AllocatorAvailabilityIdentity;
use crate::unsequenced_spill_stages::{
    GeneralizedSpillActionId, RecursiveReloadValueHomeIdentity, SpillPseudoInstructionId,
    SpillPseudoInstructionPlanIdentity, SpillPseudoOperandRewrite, SpillPseudoStorage,
    SpillPseudoStoredValue,
};
pub use identity::homed_spill_pseudo_instruction_plan_identity;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
pub use validate::validate_homed_spill_pseudo_instructions;

pub fn lower_homed_recursive_spill_pseudos(
    source: &crate::unsequenced_spill_stages::ValidatedSpillPseudoInstructions,
    homes: &crate::unsequenced_spill_stages::ValidatedRecursiveReloadValueHomes,
    policy: HomedSpillPseudoInstructionPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedHomedSpillPseudoInstructions, HomedSpillPseudoInstructionError> {
    let plan = compute::compute(source, homes, policy, budget)?;
    validate_homed_spill_pseudo_instructions(source, homes, plan)
}

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
