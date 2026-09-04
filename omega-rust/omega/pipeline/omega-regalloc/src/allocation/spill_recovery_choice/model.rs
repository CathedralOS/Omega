use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_selected_instructions::{SelectedBlockId, VirtualRegisterId};
use psi_core::MachineId;

use crate::{
    AbstractSpillInsertionIdentity, AllocationLegalityIdentity, AllocatorAvailabilityIdentity,
    LiveRangeIdentity, LiveRangePoint, SpillRecoveryWorklistIdentity, SyntheticReloadValueId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpillRecoveryChoiceIdentity(pub(crate) [u8; 32]);

impl SpillRecoveryChoiceIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpillRecoveryChoicePolicy {
    EpochOneFarthestEndThenHighestVregV1,
}

/// Recovery-victim evidence only. No field authorizes a selected rewrite,
/// logical spill operation, storage allocation, or physical realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillRecoveryChoicePlan {
    pub worklist: SpillRecoveryWorklistIdentity,
    pub abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub legality: AllocationLegalityIdentity,
    pub ranges: LiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub policy: SpillRecoveryChoicePolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub choices: Vec<SpillRecoveryVictimChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillRecoveryVictimChoice {
    pub work_item: SyntheticReloadValueId,
    pub function: usize,
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub point: LiveRangePoint,
    pub reload_class: RegisterClassId,
    pub reload_candidates: Vec<RegisterViewId>,
    pub active_residents: Vec<SpillRecoveryResident>,
    pub contenders: Vec<SpillRecoveryContender>,
    pub selected_victim: VirtualRegisterId,
    pub selected_victim_view: RegisterViewId,
    pub reclaimed_view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpillRecoveryResident {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub start: LiveRangePoint,
    pub exclusive_end: LiveRangePoint,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpillRecoveryContender {
    pub virtual_register: VirtualRegisterId,
    pub exclusive_end: LiveRangePoint,
    pub resident_view: RegisterViewId,
    pub reclaimed_view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillRecoveryChoiceReceipt {
    pub(crate) identity: SpillRecoveryChoiceIdentity,
    pub(crate) worklist: SpillRecoveryWorklistIdentity,
    pub(crate) abstract_spill_insertion: AbstractSpillInsertionIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) choice_count: usize,
    pub(crate) contender_count: usize,
}

impl SpillRecoveryChoiceReceipt {
    pub const fn identity(self) -> SpillRecoveryChoiceIdentity {
        self.identity
    }
    pub const fn worklist(self) -> SpillRecoveryWorklistIdentity {
        self.worklist
    }
    pub const fn abstract_spill_insertion(self) -> AbstractSpillInsertionIdentity {
        self.abstract_spill_insertion
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
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn choice_count(self) -> usize {
        self.choice_count
    }
    pub const fn contender_count(self) -> usize {
        self.contender_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSpillRecoveryChoices {
    pub(crate) plan: SpillRecoveryChoicePlan,
    pub(crate) receipt: SpillRecoveryChoiceReceipt,
}

impl ValidatedSpillRecoveryChoices {
    pub const fn plan(&self) -> &SpillRecoveryChoicePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> SpillRecoveryChoiceReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpillRecoveryChoiceError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedWorklistShape,
    AmbiguousWorkItem,
    FunctionMismatch {
        function: usize,
    },
    NoLivePoints {
        function: usize,
        register: u32,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    InvalidView {
        function: usize,
        view: u16,
    },
    PrefixMismatch {
        function: usize,
    },
    SecondaryPressure {
        function: usize,
        register: u32,
    },
    NoRecoverableVictim {
        function: usize,
    },
    NonCanonicalChoice,
    WorkOverflow,
    UsageMismatch,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for SpillRecoveryChoiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "spill-recovery victim choice failed: {self:?}")
    }
}

impl std::error::Error for SpillRecoveryChoiceError {}
