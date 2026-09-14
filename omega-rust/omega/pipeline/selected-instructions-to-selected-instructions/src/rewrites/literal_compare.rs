//! Literal compare/test selection on the selected CFG.
//!
//! Selection already folds an adjacent single-use `Constant` feeding an
//! equality compare into `CompareI64Zero`/`CompareI64Immediate`
//! (`scalar_graph/literal_compare`), and the selected-lowering pair-rule
//! catalog folds a pressure-nominated materialization into the immediate
//! form (`SelectedIncomingU12CompareImmediate`). Between those narrow cases
//! an ordinary `CompareI64` whose right operand is uniquely produced by a
//! `MaterializeI64` still emits the register-register form: the immediate
//! form is never selected.
//!
//! This rewrite is that compare/test selection at full selected-CFG
//! standing. `CompareI64` subtracts its right operand's bit pattern from
//! the left's and publishes the target condition state. When the right
//! operand's register is written by exactly one instruction and that
//! instruction is a `MaterializeI64` publishing the literal, the register
//! reads as that literal everywhere the compare could observe it, so the
//! comparison is `left - literal` — exactly what the immediate forms
//! compute. The compare is rewritten in place to
//! `CompareI64Immediate { immediate }`, keeping its identity, position, and
//! provenance; a literal of zero selects `CompareI64Zero`, the form
//! selection prefers for zero and the form the AArch64 `cbnz` fusion
//! recognizes.
//!
//! The admission table is deliberately narrow. Only operand 1 (the
//! subtrahend) can fold: the immediate forms always subtract the encoded
//! literal from the register operand, so a literal in operand 0 would
//! invert the ordering predicates. The literal must fit the shared
//! twelve-bit unsigned bound both target encoders enforce for
//! `CompareI64Immediate` (`u12` admits `Unsigned(v <= 4095)`; the
//! selected-lowering pair rule declares the same limit). The
//! materialization itself is retained — its register may have other
//! readers — so producer elimination stays with the producer-elimination
//! rules, not this one.
//!
//! The rewritten instruction publishes the identical implicit surface: the
//! selected row's implicit uses, definitions, and clobbers must equal the
//! compare's own, so the flag definitions conditional branches and
//! `MaterializeBoolean*` consumers observe are preserved bit-identically.
//! A second definition of the literal register — including a `UseDef`
//! rewrite or a definition carried by a terminator's instruction — refuses:
//! the literal guarantee must hold wherever the compare could read the
//! register, not only at one definition site.
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

pub use rewrite::fold_selected_literal_compare;
pub use validation::validate_literal_compare_fold;

#[cfg(test)]
mod tests;

/// An accepted literal compare/test selection with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLiteralCompare {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: LiteralCompareReceipt,
}

impl ValidatedLiteralCompare {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &LiteralCompareReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralCompareReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl LiteralCompareReceipt {
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
pub enum LiteralCompareError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedProducer,
    UnsupportedLiteral,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for LiteralCompareError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid literal compare selection: {self:?}")
    }
}

impl std::error::Error for LiteralCompareError {}
