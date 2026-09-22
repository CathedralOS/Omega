//! Optimizer module role: executable entrance. Logical spill planning and independent replay.
//!
//! `stage_register_allocation`'s runtime-spill recovery sequences this
//! boundary: over the recovery's input facts it plans the store, reload, and
//! operand-rewrite obligations for the first supported active-resident
//! pressure choice, retains the validated output on the produced allocation,
//! and re-derives it during replay. The durable record, canonical identity,
//! and versioned transport live in `register_homes::logical_spill_operations`;
//! computation, validation, and replay stay transform-local.

use crate::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedSelectedAnalysis,
    ValidatedSpillChoices,
};

mod compute;
mod validate;

#[cfg(test)]
mod tests;

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity,
    SpillChoiceIdentity,
};
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
pub use register_homes::logical_spill_operations::*;
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::FuelScheduleIdentity;
pub use validate::validate_logical_spill_operations;

/// Plan target-neutral storage, store, reload, and operand-rewrite obligations
/// for the first supported active-resident pressure choice.
pub fn plan_logical_spill_operations<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    choices: &ValidatedSpillChoices,
    policy: LogicalSpillOperationPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedLogicalSpillOperations, LogicalSpillOperationError> {
    let plan = compute::compute_terminal_logical_spill_operations(
        selected, ranges, legality, choices, policy, budget,
    )?;
    validate_logical_spill_operations(selected, ranges, legality, choices, plan)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicalSpillOperationValidationReceipt {
    pub(crate) identity: LogicalSpillOperationIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) spill_choices: SpillChoiceIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) policy: LogicalSpillOperationPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) planned_function_count: usize,
    pub(crate) store_count: usize,
    pub(crate) reload_count: usize,
    pub(crate) rewritten_use_count: usize,
}

impl LogicalSpillOperationValidationReceipt {
    pub const fn identity(self) -> LogicalSpillOperationIdentity {
        self.identity
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn spill_choices(self) -> SpillChoiceIdentity {
        self.spill_choices
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
    pub const fn policy(self) -> LogicalSpillOperationPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn planned_function_count(self) -> usize {
        self.planned_function_count
    }
    pub const fn store_count(self) -> usize {
        self.store_count
    }
    pub const fn reload_count(self) -> usize {
        self.reload_count
    }
    pub const fn rewritten_use_count(self) -> usize {
        self.rewritten_use_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLogicalSpillOperations {
    pub(crate) plan: LogicalSpillOperationPlan,
    pub(crate) receipt: LogicalSpillOperationValidationReceipt,
}

impl ValidatedLogicalSpillOperations {
    pub const fn plan(&self) -> &LogicalSpillOperationPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> LogicalSpillOperationValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicalSpillOperationError {
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
    UnsupportedVictimRole {
        function: usize,
        register: u32,
    },
    UnsupportedScalarType {
        function: usize,
        register: u32,
    },
    UnsupportedOrigin {
        function: usize,
        register: u32,
    },
    UnsupportedRangeShape {
        function: usize,
        register: u32,
    },
    IncomingDefinitionMismatch {
        function: usize,
        register: u32,
    },
    FutureFixedUse {
        function: usize,
        register: u32,
    },
    NoFutureUse {
        function: usize,
        register: u32,
    },
    FutureUseMismatch {
        function: usize,
        register: u32,
    },
    IdentifierOverflow {
        function: usize,
    },
    NonCanonicalStorageIds {
        function: usize,
    },
    DecisionMismatch {
        function: usize,
    },
    UsageMismatch,
}

impl std::fmt::Display for LogicalSpillOperationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal logical spill planning failed: {self:?}"
        )
    }
}

impl std::error::Error for LogicalSpillOperationError {}
