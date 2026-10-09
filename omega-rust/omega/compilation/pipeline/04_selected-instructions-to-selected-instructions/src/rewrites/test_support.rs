//! Shared source-to-rewrite fixtures for cross-stage allocation controls.
//!
//! The rewrite corpus keeps one fixture language across rule families: the
//! dominant work budget, measured step-boundary budgets, and the
//! constraint-row instruction builder every hand-built plan shares, so a rule
//! file holds only the identifiers and shapes that distinguish its subject.

pub use crate::rewrites::runtime_spill::test_support::runtime_spill_source;
use optimization_core::OptimizationWorkBudget;

pub use crate::rewrites::allocation_recovery::pressure_rematerialization::tests::{
    multiple_use::exercise_multiple_use_rematerialization,
    sole_use::exercise_single_use_rematerialization,
};

/// The generous fixture budget shared by the rewrite corpus: wide enough that
/// no exercised plan reaches it, so budget assertions stay about the measured
/// axes rather than incidental totals.
pub fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap()
}

/// A budget carrying `steps` measured steps on a one-everything-else profile,
/// for the measured validation-step boundary legs: the exact count admits,
/// `measured_step_budget(steps - 1)` starves.
pub(crate) fn measured_step_budget(steps: u64) -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1, 1, steps, 1, 1).unwrap()
}
