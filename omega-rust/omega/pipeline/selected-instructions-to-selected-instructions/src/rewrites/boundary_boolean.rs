//! Boundary-decided predicate materialization on the selected CFG.
//!
//! The constant-flag cluster decides a flag reader when the compare it
//! observes is compile-time constant on both operands
//! (`rewrites/constant_boolean` materializes the decided boolean,
//! `rewrites/constant_branch` jumps the decided edge), and the literal
//! selection folds (the selected-lowering compare pair rules and
//! `rewrites/literal_minuend`) only change the compare's form. Between those cases a compare whose
//! single known operand sits at a carrier-domain pole still reaches a
//! reader whose predicate the pole decides alone: `x < 0` unsigned and
//! `x < i64::MIN` signed can never hold, `0 <= x` unsigned and
//! `x <= i64::MAX` signed always hold, and `x <= 0` unsigned — like
//! `u64::MAX <= x`, `x <= i64::MIN`, or `i64::MAX <= x` — collapses to
//! equality at the pole. The surviving operand needs no producer at all.
//!
//! This rewrite is that compare/test selection at full selected-CFG
//! standing. A `MaterializeBooleanU64LessThan`,
//! `MaterializeBooleanI64LessThan`, `MaterializeBooleanU64LessOrEqual`, or
//! `MaterializeBooleanI64LessOrEqual` whose every implicit condition-state
//! use resolves to one compare — the last flag event on every path reaching
//! the materialization, through the shared least-fixpoint entry-event walk
//! — is admitted when the compare's operand audit yields a pole that fixes
//! the predicate. The compare is any of the three forms: `CompareI64`
//! resolves each side through its unique `MaterializeI64` producer where
//! one exists, `CompareI64Immediate` carries its encoded literal, and
//! `CompareI64Zero` reads as `left - 0`. A side nothing pins is simply
//! unknown; the pole must decide without it. The strict less-than forms
//! fold at the far pole only — `0 < x` unsigned is `x != 0` and `x < MAX`
//! is `x != MAX`, negations no boolean kind materializes — while the
//! less-or-equal forms at the near pole are exactly the equality condition
//! (`x <= 0` unsigned holds precisely when `x == 0`), so the reader's kind
//! rewrites to `MaterializeBooleanEqual` over the identical published
//! state: the flag-reading kinds share one constraint row, so operands,
//! implicit uses, and clobbers carry over verbatim. A compare reading the
//! same register twice is the identity case the constant family already
//! decides and stays refused here.
//!
//! A decided predicate rewrites the reader to the target's own
//! `MaterializeI64` of the outcome — the same emission the constant fold
//! uses — dropping the flag uses while the compare keeps publishing flag
//! state for every other reader. An equality collapse keeps the reader a
//! flag consumer: only its kind field changes. Either way the instruction
//! keeps its identity, position, result register, and provenance.
//!
//! Proposal and independent replay share only the admission predicates and
//! the rewritten-instruction constructor. Validation re-derives the
//! admitted pair from the source, requires the proposed instruction to
//! equal the reconstructed form, and restores the complete source by
//! content: every other instruction, register, roster row, call, and
//! settlement is retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::fold_selected_boundary_boolean;
pub use validation::validate_boundary_boolean_fold;

#[cfg(test)]
mod tests;

/// An accepted boundary-decided predicate materialization with its replay
/// receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedBoundaryBoolean {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: BoundaryBooleanReceipt,
}

impl ValidatedBoundaryBoolean {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &BoundaryBooleanReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryBooleanReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl BoundaryBooleanReceipt {
    pub const fn source_selected(&self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn transformed_selected(&self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn optimization_unit(&self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(&self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryBooleanError {
    SourceMismatch,
    /// The named instruction is not one of the four ordering
    /// `MaterializeBoolean*` kinds in the emitted `[def]` flag-reader
    /// shape — `MaterializeBooleanEqual` reads a condition no pole can
    /// decide.
    UnsupportedInstruction,
    /// The resolved compare carries no operand pole that decides the
    /// reader's predicate: both sides unknown, a non-pole literal, the
    /// identity `register - register` case the constant family owns, or a
    /// strict less-than collapsed to a negation no boolean kind
    /// materializes.
    UnsupportedLiteral,
    /// A flag observation the resolution cannot pin to the compare: a
    /// clobber, a different instruction's event, unknown entry state, an
    /// eventless path, paths that disagree, a unit the compare does not
    /// publish, or a compare operand position that is not a plain use.
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for BoundaryBooleanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid boundary boolean fold: {self:?}")
    }
}

impl std::error::Error for BoundaryBooleanError {}
