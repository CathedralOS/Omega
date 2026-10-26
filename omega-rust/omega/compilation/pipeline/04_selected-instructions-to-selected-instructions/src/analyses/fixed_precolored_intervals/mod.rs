//! Optimizer module role: executable entrance. Fixed/precolored point-interval analysis.
//!
//! This boundary resolves existing fixed constraints to exact half-open point
//! intervals. It does not choose a home, insert a copy, or split a live range.

mod compute;
mod validate;

use crate::WorkDefect;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_homes::FixedPrecoloredIntervalPolicy;
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlan,
    FixedPrecoloredIntervalPlanIdentity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::LiveRangeIdentity;
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) fn analyze_fixed_precolored_intervals(
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    policy: FixedPrecoloredIntervalPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedPrecoloredIntervals, FixedPrecoloredIntervalError> {
    let plan = compute::compute(ranges, legality, policy, budget)?;
    validate::seal_fixed_precolored_intervals(plan)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredIntervalValidationReceipt {
    pub(crate) identity: FixedPrecoloredIntervalPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) policy: FixedPrecoloredIntervalPolicy,
    pub(crate) usage: OptimizationWorkUsage,
}

impl FixedPrecoloredIntervalValidationReceipt {
    receipt_accessor_rows!(FixedPrecoloredIntervalValidationReceipt {
        identity: FixedPrecoloredIntervalPlanIdentity,
        policy: FixedPrecoloredIntervalPolicy,
        usage: OptimizationWorkUsage,
    });
    shared_analysis_identity_accessors!(pub);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFixedPrecoloredIntervals {
    pub(crate) plan: FixedPrecoloredIntervalPlan,
    pub(crate) receipt: FixedPrecoloredIntervalValidationReceipt,
}

impl ValidatedFixedPrecoloredIntervals {
    pub const fn plan(&self) -> &FixedPrecoloredIntervalPlan {
        &self.plan
    }
    receipt_accessor_rows!(ref ValidatedFixedPrecoloredIntervals {
        receipt: FixedPrecoloredIntervalValidationReceipt,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedPrecoloredIntervalError {
    RootMismatch,
    FunctionMismatch {
        function: usize,
    },
    RegisterMismatch {
        function: usize,
        register: u32,
    },
    ConstraintPointMissing {
        function: usize,
        register: u32,
        point: u32,
    },
    ConstraintViewMismatch {
        function: usize,
        register: u32,
        view: u16,
    },
    UnsupportedEarlyClobberFixedConstraint {
        function: usize,
        register: u32,
        instruction: u32,
        operand: u16,
    },
    IntervalOverflow {
        function: usize,
        register: u32,
        point: u32,
    },
    /// Bounded work accounting refused: the logical receipt overflowed or
    /// exceeded the selected budget.
    Work(WorkDefect),
    UsageMismatch,
    NonCanonicalFunctions,
}

failed_display!(
    FixedPrecoloredIntervalError,
    "fixed/precolored interval analysis failed"
);

impl std::error::Error for FixedPrecoloredIntervalError {}
