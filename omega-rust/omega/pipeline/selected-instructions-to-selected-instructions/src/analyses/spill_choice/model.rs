use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, SpillChoiceIdentity,
    SpillChoicePlan, SpillChoicePolicy,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::LiveRangeIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpillChoiceValidationReceipt {
    pub(crate) identity: SpillChoiceIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) policy: SpillChoicePolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) choice_count: usize,
    pub(crate) contender_count: usize,
}

impl SpillChoiceValidationReceipt {
    pub const fn identity(self) -> SpillChoiceIdentity {
        self.identity
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
    pub const fn policy(self) -> SpillChoicePolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn choice_count(self) -> usize {
        self.choice_count
    }
    pub const fn contender_count(self) -> usize {
        self.contender_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSpillChoices {
    pub(crate) plan: SpillChoicePlan,
    pub(crate) receipt: SpillChoiceValidationReceipt,
}

impl ValidatedSpillChoices {
    pub const fn plan(&self) -> &SpillChoicePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> SpillChoiceValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpillChoiceError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTiedOperands {
        function: usize,
    },
    UnsupportedEarlyClobber {
        function: usize,
    },
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    FunctionMismatch {
        function: usize,
    },
    VirtualRegisterMismatch {
        function: usize,
        register: u32,
    },
    UnresolvedEntryTransitions {
        function: usize,
        register: u32,
    },
    NoLivePoints {
        function: usize,
        register: u32,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
    },
    NoCommonCandidate {
        function: usize,
        register: u32,
    },
    UnknownOrIncompatibleView {
        function: usize,
        register: u32,
        view: u16,
    },
    UnsupportedPressureShape {
        function: usize,
        register: u32,
    },
    ChoiceMismatch {
        function: usize,
    },
    UsageMismatch,
}

impl std::fmt::Display for SpillChoiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal recovery-victim choice failed: {self:?}"
        )
    }
}

impl std::error::Error for SpillChoiceError {}
