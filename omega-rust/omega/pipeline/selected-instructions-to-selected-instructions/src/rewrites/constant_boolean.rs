//! Constant condition materialization on the selected CFG.
//!
//! `rewrites/literal_compare` and `rewrites/literal_minuend` select the
//! immediate or zero compare form when one operand is a uniquely
//! materialized literal. Between them an operand gap remains: when *every*
//! input a compare reads is compile-time known, the condition state the
//! compare publishes is itself a constant — and a `MaterializeBoolean*`
//! observing that state is a `MaterializeI64` of the predicate outcome.
//! Nothing selects that materialization: the boolean forms are the only
//! flag-to-register readers in the selected catalog, and the literal
//! compare folds stop at the compare's own boundary.
//!
//! This rewrite is that compare/test selection at full selected-CFG
//! standing. A compare's flag state is constant in the cases the selected
//! forms can express: `CompareI64` whose two operand registers are each
//! uniquely produced by a `MaterializeI64` (any sixty-four-bit literal —
//! nothing is re-encoded, so no immediate bound applies), `CompareI64`
//! whose operands name the same register (`register - register` is zero on
//! every lane), `CompareI64Immediate` on a uniquely materialized register
//! (`literal - immediate`), and `CompareI64Zero` on a uniquely
//! materialized register (`literal - 0`). A `MaterializeBooleanEqual`,
//! `MaterializeBooleanU64LessThan`, `MaterializeBooleanI64LessThan`,
//! `MaterializeBooleanU64LessOrEqual`, or
//! `MaterializeBooleanI64LessOrEqual` whose implicit flag uses all reach
//! from that compare — each used unit's nearest preceding in-block
//! definition or clobber is the compare, and the unit is among its
//! published definitions — is rewritten in place to `MaterializeI64`
//! carrying `0` or `1` as the predicate dictates, keeping its identity,
//! position, result register, and provenance.
//!
//! The rewritten instruction's implicit surface legitimately narrows: the
//! materialize row declares no unit traffic, so the flag uses the boolean
//! carried are dropped. That is the semantics of the fold — the result no
//! longer observes condition state because the state it observed is a
//! constant. The compare itself is retained: terminator branches and any
//! other reached readers still observe the same flag definitions, and
//! producer elimination stays with the producer-elimination rules.
//!
//! The admission table is deliberately bounded. The flag walk resolves a
//! used unit only to a definition or clobber in the materialization's own
//! block before its position: a flag reaching in from a predecessor edge,
//! or a unit with no in-block event, refuses — proving cross-block flag
//! custody belongs to a later family. A used unit whose reaching event is
//! a clobber, a non-compare definition, or a second compare refuses: the
//! observed state is not provably constant. The compare's literal inputs
//! need the same unique-producer guarantee the literal folds use — exactly
//! one defining instruction in the function, a clean `[def]`
//! `MaterializeI64` with no unit traffic — and each literal must fit the
//! sixty-four-bit register pattern it publishes. Ordering predicates
//! evaluate in the compare's own direction (`left - right`), so no swapped
//! subtraction or consumer audit is needed.
//!
//! Proposal and independent replay share only the admission predicates and
//! the rewritten-instruction constructor. Validation re-derives the
//! admitted materialization from the source, requires the proposed
//! instruction to equal the reconstructed form, and restores the complete
//! source by content: every other instruction, register, roster row, call,
//! and settlement is retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::fold_selected_constant_boolean;
pub use validation::validate_constant_boolean_fold;

#[cfg(test)]
mod tests;

/// An accepted constant condition materialization with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedConstantBoolean {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ConstantBooleanReceipt,
}

impl ValidatedConstantBoolean {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &ConstantBooleanReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantBooleanReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl ConstantBooleanReceipt {
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
pub enum ConstantBooleanError {
    SourceMismatch,
    UnsupportedInstruction,
    /// A flag observation the constant fold cannot reproduce: a used unit
    /// whose nearest in-block event is a clobber, a non-compare definition,
    /// or a different instruction than its siblings; a used unit with no
    /// in-block reaching event; or the flag-free boolean shape that has no
    /// condition to evaluate.
    UnsupportedUse,
    UnsupportedProducer,
    UnsupportedLiteral,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ConstantBooleanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid constant boolean fold: {self:?}")
    }
}

impl std::error::Error for ConstantBooleanError {}
