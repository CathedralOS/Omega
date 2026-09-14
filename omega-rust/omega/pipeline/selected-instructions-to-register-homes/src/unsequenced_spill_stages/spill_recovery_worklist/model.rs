use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::SelectedBlockId;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::{
    AbstractSpillInsertionIdentity, AllocationLegalityIdentity, AllocatorAvailabilityIdentity,
    LiveRangeIdentity, LiveRangePoint, LogicalReloadValueId, LogicalSpillOperationIdentity,
    ReloadValueHomeError, ReloadValueHomePolicy, SyntheticReloadValueId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpillRecoveryWorklistIdentity(pub(crate) [u8; 32]);

impl SpillRecoveryWorklistIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// V1 admits exactly one epoch-one work item sourced from one validated
/// reload-pressure failure. Later epochs require a new explicit policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpillRecoveryWorklistPolicy {
    SingleReloadPressureEpochOneV1,
}

/// Compiler-private scheduling custody only. The plan names work that a later
/// recovery boundary may attempt; it does not choose a spill victim, allocate
/// a view, or rewrite any selected program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillRecoveryWorklistPlan {
    pub abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub logical_spill_operations: LogicalSpillOperationIdentity,
    pub legality: AllocationLegalityIdentity,
    pub ranges: LiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub reload_home_policy: ReloadValueHomePolicy,
    /// Budget under which the independent source failure must reproduce.
    pub reload_home_budget: OptimizationWorkBudget,
    pub policy: SpillRecoveryWorklistPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub epochs: Vec<SpillRecoveryEpoch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillRecoveryEpoch {
    pub epoch: u32,
    pub work_items: Vec<SpillRecoveryWorkItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillRecoveryWorkItem {
    /// Compiler-private namespace only; never a selected `VirtualRegisterId`.
    pub synthetic: SyntheticReloadValueId,
    pub machine: MachineId,
    pub source_reload: LogicalReloadValueId,
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub class: RegisterClassId,
    /// Complete canonical view domain that was blocked at `start`.
    pub candidates: Vec<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillRecoveryWorklistReceipt {
    pub(crate) identity: SpillRecoveryWorklistIdentity,
    pub(crate) abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub(crate) logical_spill_operations: LogicalSpillOperationIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) epoch_count: usize,
    pub(crate) work_item_count: usize,
}

impl SpillRecoveryWorklistReceipt {
    pub const fn identity(self) -> SpillRecoveryWorklistIdentity {
        self.identity
    }
    pub const fn abstract_spill_insertion(self) -> AbstractSpillInsertionIdentity {
        self.abstract_spill_insertion
    }
    pub const fn logical_spill_operations(self) -> LogicalSpillOperationIdentity {
        self.logical_spill_operations
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
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
    pub const fn epoch_count(self) -> usize {
        self.epoch_count
    }
    pub const fn work_item_count(self) -> usize {
        self.work_item_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSpillRecoveryWorklist {
    pub(crate) plan: SpillRecoveryWorklistPlan,
    pub(crate) receipt: SpillRecoveryWorklistReceipt,
}

impl ValidatedSpillRecoveryWorklist {
    pub const fn plan(&self) -> &SpillRecoveryWorklistPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> SpillRecoveryWorklistReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpillRecoveryWorklistError {
    RootMismatch,
    UnsupportedPolicy,
    ReloadPressureRequired,
    SourceReloadHome(ReloadValueHomeError),
    TriggerMismatch {
        function: usize,
    },
    InvalidCandidateDomain {
        function: usize,
    },
    IntervalOverflow {
        function: usize,
    },
    NonCanonicalWorklist,
    WorkOverflow,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for SpillRecoveryWorklistError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "spill-recovery worklist seeding failed: {self:?}"
        )
    }
}

impl std::error::Error for SpillRecoveryWorklistError {}
