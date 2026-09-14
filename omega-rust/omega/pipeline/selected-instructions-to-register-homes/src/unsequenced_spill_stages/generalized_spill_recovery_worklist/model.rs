use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{SelectedBlockId, SelectedInstructionPlanIdentity};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::{
    AbstractSpillInsertionIdentity, AllocationLegalityIdentity, AllocatorAvailabilityIdentity,
    GeneralizedReloadCoexistingHome, GeneralizedReloadValueHomeIdentity, GeneralizedSpillActionId,
    GeneralizedSpillActionSource, GeneralizedSpillInsertionIdentity, LiveRangeIdentity,
    LiveRangePoint, SpillRecoveryActionIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeneralizedSpillRecoveryWorklistIdentity(pub(crate) [u8; 32]);

impl GeneralizedSpillRecoveryWorklistIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// V1 consumes only the epoch-one pressure retained by generalized reload-home
/// reanalysis and creates one epoch-two item per pressured function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeneralizedSpillRecoveryWorklistPolicy {
    EpochOnePressureToEpochTwoV1,
}

/// Compiler-private work identity only. It is neither a selected virtual
/// register nor a generalized spill action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeneralizedSpillRecoveryWorkItemId {
    pub epoch: u32,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryWorklistPlan {
    pub reload_value_homes: GeneralizedReloadValueHomeIdentity,
    pub generalized_spill_insertion: GeneralizedSpillInsertionIdentity,
    pub abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub spill_recovery_actions: SpillRecoveryActionIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: GeneralizedSpillRecoveryWorklistPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionGeneralizedSpillRecoveryWorklist>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionGeneralizedSpillRecoveryWorklist {
    pub machine: MachineId,
    pub item: Option<GeneralizedSpillRecoveryWorkItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryWorkItem {
    pub id: GeneralizedSpillRecoveryWorkItemId,
    pub source_pressure: GeneralizedSpillActionId,
    pub source: GeneralizedSpillActionSource,
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub class: RegisterClassId,
    pub candidates: Vec<RegisterViewId>,
    pub blocking_homes: Vec<GeneralizedReloadCoexistingHome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralizedSpillRecoveryWorklistReceipt {
    pub(crate) identity: GeneralizedSpillRecoveryWorklistIdentity,
    pub(crate) reload_value_homes: GeneralizedReloadValueHomeIdentity,
    pub(crate) generalized_spill_insertion: GeneralizedSpillInsertionIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) work_item_count: usize,
    pub(crate) blocking_home_count: usize,
}

impl GeneralizedSpillRecoveryWorklistReceipt {
    pub const fn identity(self) -> GeneralizedSpillRecoveryWorklistIdentity {
        self.identity
    }

    pub const fn reload_value_homes(self) -> GeneralizedReloadValueHomeIdentity {
        self.reload_value_homes
    }

    pub const fn generalized_spill_insertion(self) -> GeneralizedSpillInsertionIdentity {
        self.generalized_spill_insertion
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

    pub const fn work_item_count(self) -> usize {
        self.work_item_count
    }

    pub const fn blocking_home_count(self) -> usize {
        self.blocking_home_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedGeneralizedSpillRecoveryWorklist {
    pub(crate) plan: GeneralizedSpillRecoveryWorklistPlan,
    pub(crate) receipt: GeneralizedSpillRecoveryWorklistReceipt,
}

impl ValidatedGeneralizedSpillRecoveryWorklist {
    pub const fn plan(&self) -> &GeneralizedSpillRecoveryWorklistPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> GeneralizedSpillRecoveryWorklistReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneralizedSpillRecoveryWorklistError {
    RootMismatch,
    UnsupportedPolicy,
    PressureRequired,
    InvalidSourceOutcomes {
        function: usize,
    },
    EpochOverflow {
        function: usize,
    },
    NonCanonicalWorklist {
        function: usize,
    },
    WorkOverflow,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for GeneralizedSpillRecoveryWorklistError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "generalized spill-recovery worklist seeding failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralizedSpillRecoveryWorklistError {}
