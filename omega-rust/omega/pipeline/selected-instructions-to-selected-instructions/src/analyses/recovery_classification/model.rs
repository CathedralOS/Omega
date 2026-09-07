use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, SpillChoiceIdentity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{LiveRangeIdentity, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryClassificationValidationReceipt {
    pub(crate) identity: RecoveryClassificationIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) spill_choices: SpillChoiceIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) policy: RecoveryClassificationPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) classification_count: usize,
    pub(crate) immediate_candidate_count: usize,
}

impl RecoveryClassificationValidationReceipt {
    pub const fn identity(self) -> RecoveryClassificationIdentity {
        self.identity
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn spill_choices(self) -> SpillChoiceIdentity {
        self.spill_choices
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
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
    pub const fn policy(self) -> RecoveryClassificationPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn classification_count(self) -> usize {
        self.classification_count
    }
    pub const fn immediate_candidate_count(self) -> usize {
        self.immediate_candidate_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRecoveryClassifications {
    pub(crate) plan: RecoveryClassificationPlan,
    pub(crate) receipt: RecoveryClassificationValidationReceipt,
}

impl ValidatedRecoveryClassifications {
    pub const fn plan(&self) -> &RecoveryClassificationPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> RecoveryClassificationValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryClassificationError {
    RootMismatch,
    UnsupportedPolicy,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    FunctionMismatch {
        function: usize,
    },
    ChoiceMismatch {
        function: usize,
    },
    VictimMismatch {
        function: usize,
        register: u32,
    },
    ClassificationMismatch {
        function: usize,
    },
    UsageMismatch,
}

impl std::fmt::Display for RecoveryClassificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal recovery classification failed: {self:?}"
        )
    }
}

impl std::error::Error for RecoveryClassificationError {}
