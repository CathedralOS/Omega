//! Left-operand literal folding on the selected CFG.
//!
//! The selected-lowering compare pair rules fold a `CompareI64` whose right
//! operand is a uniquely materialized literal into
//! `CompareI64Immediate`/`CompareI64Zero`,
//! because the immediate forms always subtract the encoded literal from the
//! register operand. A `CompareI64` whose left operand — the minuend — is
//! the materialized literal cannot use that rule: rewriting it in place
//! computes `register - literal`, the swapped subtraction, whose ordering
//! predicates are the inverse of the source's. Selection's adjacent
//! equality fold and the selected-lowering compare pair rule both stay
//! inside the right-operand case, so the left-literal compare still emits
//! the register-register form.
//!
//! This rewrite is the left-operand leg at full selected-CFG standing. The
//! zero condition is symmetric under the swap — `literal - register`
//! produces it exactly when `register - literal` does — so the swapped
//! immediate form publishes condition state that every equality-sensing
//! consumer observes identically. Ordering consumers observe the swap:
//! `MaterializeBoolean*LessThan`, `MaterializeBoolean*LessOrEqual`, and the
//! predicate-aware `ConditionalBranch*LessThan` terminators read flags that
//! now describe `register` against `literal`, and no reversed predicate form
//! exists to compensate. The admission table therefore splits on what the
//! compare's flag definitions can reach: an instruction whose implicit uses
//! intersect those units is admitted only when its kind is
//! `MaterializeBooleanEqual` or `ConditionalBranchNonZero`, the two kinds
//! that read the zero condition alone. The walk follows each defined unit
//! forward from the compare — through body and terminator-carried
//! instructions, then along the terminator's successor edges — until a
//! redefinition or clobber ends it; a unit still live into a successor edge
//! resumes the scan in the target block, so a flag reader on another block's
//! path is audited the same way. When both operands are the same register
//! the subtraction is literally identical, every flag bit survives, and the
//! consumer audit is unnecessary.
//!
//! The remaining admission table matches the sibling family's. The literal
//! must fit the shared twelve-bit unsigned bound both target encoders
//! enforce for `CompareI64Immediate`; zero selects `CompareI64Zero`. The
//! literal register needs exactly one defining instruction, a clean
//! `[def]` `MaterializeI64`, and the materialization itself is retained —
//! its register may have other readers, so producer elimination stays with
//! the producer-elimination rules. The rewritten instruction keeps the
//! compare's identity, position, provenance, and published flag surface:
//! the selected row must declare a single `Use` at the surviving
//! subtrahend's class and publish exactly the implicit uses, definitions,
//! and clobbers the compare carried.
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

pub use rewrite::fold_selected_literal_minuend;
pub use validation::validate_literal_minuend_fold;

#[cfg(test)]
mod tests;

/// An accepted left-operand literal fold with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLiteralMinuend {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: LiteralMinuendReceipt,
}

impl ValidatedLiteralMinuend {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &LiteralMinuendReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralMinuendReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl LiteralMinuendReceipt {
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
pub enum LiteralMinuendError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedProducer,
    UnsupportedLiteral,
    /// A flag observation the swapped comparison cannot reproduce: a reached
    /// reader whose kind is not equality-sensing, a live flag unit escaping
    /// to a successor the plan does not name, or the surviving input missing
    /// from the register roster.
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for LiteralMinuendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid literal minuend fold: {self:?}")
    }
}

impl std::error::Error for LiteralMinuendError {}
