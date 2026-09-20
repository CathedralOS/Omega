//! Equivalent compare removal on the selected CFG.
//!
//! A compare is equivalent to its shadow when the flag state it publishes
//! is already the state every path into it observes — the same condition
//! the strict [`super::redundant`] family asks — but the proof compares
//! *values* rather than forms: a shadow may be any of the three compare
//! kinds and may read different operand registers, provided each semantic
//! side of the subtraction provably coincides. The subtraction is the whole
//! publication: `CompareI64` computes `left - right` from two registers,
//! `CompareI64Immediate` subtracts its encoded literal from a register, and
//! `CompareI64Zero` reads as `left - 0`, and two compares whose sides carry
//! the same bits republish the same flag state whatever their forms.
//!
//! The grammar the family admits is deliberately exact. A side coincides by
//! register identity — the same virtual register on both compares — or by
//! literal: the register's unique producer is a `MaterializeI64` carrying
//! exactly the bits the other side encodes, a guarantee the shared
//! unique-producer audit extends function-wide rather than at one site, so
//! a literal-pinned side needs no path stability at all. Subtraction
//! direction is part of the grammar: the left sides must coincide and the
//! right sides must coincide — a swapped operand pair is a different
//! publication unless both sides independently resolve to equal values —
//! and an immediate payload decodes only through the unsigned encoding the
//! target forms admit. A site that is not a canonical compare, a clobber
//! where a definition is required, or a side that resolves to no common
//! value all refuse.
//!
//! The motivating shape is what the selection folds leave behind.
//! the compare pair rules rewrite `CompareI64` into `CompareI64Immediate` or
//! `CompareI64Zero` while the materialization stays for its other readers;
//! a later compare — register-form, or immediate-form against the same
//! literal — then republishes the subtraction the folded shadow already
//! computes, and the strict family refuses it only because the forms and
//! operand registers differ. The family is also where a cross-form diamond
//! lands: one arm's register-form shadow and the other arm's immediate-form
//! shadow are the same reaching set only when the unit's flag value, not
//! the instruction kind, is what they agree on.
//!
//! Two audits carry the proof. First, every unit in the compare's implicit
//! definitions and clobbers resolves through the shared condition-state
//! reaching walk — the last in-block event when one precedes the compare,
//! otherwise the least-fixpoint entry set over the predecessor cone — and
//! every site in a defined unit's set must be a canonical compare that
//! defines the unit and computes the same subtraction; a clobbered unit
//! keeps the strict rule that every site leaves it unspecified. An entry
//! unknown, an eventless path, or a divergent site all refuse. Second, the
//! operand audit narrows to the registers the two compares share by
//! identity: only those can drift between a shadow and the compare, so the
//! backward walk — body positions, terminator-carried instructions, and
//! the parameter positions every crossed edge's transports define — refuses
//! any write to a shared register and nothing more.
//!
//! Cycles need no separate rule, exactly as in the strict family: the
//! compare's own earlier execution is a trivially equivalent shadow, and
//! the backward audit covers the loop positions the cyclic segment crosses.
//!
//! Removing the compare shortens its block's instruction vector, so
//! boundary settlements positioned after it shift one ordinal earlier —
//! the same remap the dead and strict families apply — while positions at
//! or before it are untouched. The compare's register uses simply
//! disappear.
//!
//! Proposal and independent replay share only the admission predicates and
//! the settlement remap. Validation consumes the proposed program, requires
//! the function to equal the independently computed removal, and restores
//! the complete source by content — every other instruction, register,
//! roster row, call, and settlement included.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::remove_equivalent_compare;
pub use validation::validate_equivalent_compare;

#[cfg(test)]
mod tests;

/// An accepted equivalent-compare removal with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedEquivalentCompare {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: EquivalentCompareReceipt,
}

impl ValidatedEquivalentCompare {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &EquivalentCompareReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquivalentCompareReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl EquivalentCompareReceipt {
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
pub enum EquivalentCompareError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for EquivalentCompareError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid equivalent-compare removal: {self:?}")
    }
}

impl std::error::Error for EquivalentCompareError {}
