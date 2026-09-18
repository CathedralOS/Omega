//! Literal operand selection for exact arithmetic on the selected CFG.
//!
//! Selection always emits `ExactAddI64` and `ExactSubtractI64` in their
//! register-register forms (`scalar_graph` binds the `add_i64`/`subtract_i64`
//! rows), and the selected-lowering pair-rule catalog folds a
//! pressure-nominated materialization into the immediate forms
//! (`EXACT_ADD_IMMEDIATE_U12`, `EXACT_ADD_LEFT_IMMEDIATE_U12`, and
//! `EXACT_SUBTRACT_IMMEDIATE_U12`). Between those narrow cases an ordinary
//! `ExactAddI64`/`ExactSubtractI64` whose operand is uniquely produced by a
//! `MaterializeI64` still emits the register-register form: the immediate
//! form is never selected.
//!
//! This rewrite is that operand selection at full selected-CFG standing,
//! the exact-arithmetic sibling of `literal_compare`. `ExactAddI64` adds
//! its two operand bit patterns exactly and `ExactSubtractI64` subtracts
//! operand 1 from operand 0 exactly, each under the source proof obligation
//! and accepted fact its kind fields carry. When one operand's register is
//! written by exactly one instruction and that instruction is a
//! `MaterializeI64` publishing the literal, the register reads as that
//! literal everywhere the operation could observe it, so the computation
//! is `surviving + literal` or `surviving - literal` — exactly what the
//! immediate forms encode. The instruction is rewritten in place to
//! `ExactAddI64Immediate`/`ExactSubtractI64Immediate` carrying the literal
//! and the identical obligation and accepted fact, keeping its identity,
//! position, and provenance, while the materialization is retained for the
//! literal register's other readers.
//!
//! The admission table is deliberately narrow, matching the pair rules'
//! grammars. Operand 1 — the addend/subtrahend position — folds for both
//! kinds: the immediate forms always apply the encoded literal to the
//! register operand in the register-register form's own direction.
//! Operand 0 folds only for `ExactAddI64`: exact addition commutes, so
//! `literal + x` rewrites to the same `x + literal` form, while no
//! `literal - x` immediate form exists for `ExactSubtractI64` to select.
//! The literal must fit the shared twelve-bit unsigned bound both target
//! encoders enforce for the immediate arithmetic forms (`u12` admits
//! `Unsigned(v <= 4095)`; the pair rules declare the same limit).
//!
//! The rewritten instruction must publish the identical implicit surface:
//! the selected row's implicit uses, definitions, and clobbers must equal
//! the operation's own, and its declared row must publish the same surface
//! it actually carries — so nothing the instruction observed or destroyed
//! silently changes with the form. That gate admits the subtract fold only
//! where the target's two rows agree: aarch64's `sub` realizations write
//! no flags on either form, while x86-64's register-register
//! `ExactSubtractI64` row declares the `rflags` clobber its flag-writing
//! variants produce and the `lea`-encoded `ExactSubtractI64Immediate` row
//! preserves flags — the surfaces differ and admission refuses there.
//! A second definition of the literal register — including a `UseDef`
//! rewrite or a definition carried by a terminator's instruction —
//! refuses: the literal guarantee must hold wherever the operation could
//! read the register, not only at one definition site.
//!
//! Proposal and independent replay share only the admission predicates and
//! the rewritten-instruction constructor. Validation re-derives the
//! admitted operand from the source, requires the proposed instruction to
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

pub use rewrite::fold_selected_literal_arithmetic;
pub use validation::validate_literal_arithmetic_fold;

#[cfg(test)]
mod tests;

/// An accepted literal arithmetic selection with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLiteralArithmetic {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: LiteralArithmeticReceipt,
}

impl ValidatedLiteralArithmetic {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &LiteralArithmeticReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralArithmeticReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl LiteralArithmeticReceipt {
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
pub enum LiteralArithmeticError {
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

impl std::fmt::Display for LiteralArithmeticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid literal arithmetic selection: {self:?}")
    }
}

impl std::error::Error for LiteralArithmeticError {}
