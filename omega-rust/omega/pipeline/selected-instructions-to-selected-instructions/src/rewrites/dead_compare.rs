//! Dead flag-publisher removal on the selected CFG.
//!
//! The three compare forms — `CompareI64`, `CompareI64Immediate`, and
//! `CompareI64Zero` — exist only to publish target condition state: they
//! read register operands, define the flag units their constraint row
//! declares, and carry no scalar result. When every flag unit a compare
//! publishes dies unobserved — every forward path reaching a redefinition
//! or clobber before any reader, or leaving the function entirely — the
//! compare computes state nothing can observe and the instruction can be
//! removed outright.
//!
//! That is the shape the constant-flag cluster leaves behind: once
//! `constant_boolean` and `constant_branch` have folded every
//! `MaterializeBoolean*` materialization and conditional-branch terminator
//! reading a constant-operand compare, the compare still publishes flag
//! state no reader remains to consume. The same shape arises without the
//! folds whenever a later flag event overwrites the compare's surface
//! before any reader on every path — back-to-back compares feeding
//! different branches, or a compare on a path that always re-compares.
//!
//! The admission is deliberately narrow. The named instruction must be one
//! of the three compare kinds in its canonical emitted shape — plain `Use`
//! operands only, no implicit uses, and an implicit surface identical to
//! its own constraint row's — because only those kinds carry no effect
//! beyond the published flags. Every unit in the compare's implicit
//! definitions and clobbers must die unread: the walk scans forward from
//! the instruction after the compare, refusing any implicit use it reaches
//! and ending the unit's live range at the first redefinition or clobber;
//! a unit still live at a block's end resumes at the head of each
//! successor block, a successor edge naming a block the function does not
//! contain refuses, and a path that runs out of successors — a return or
//! hosted exit — leaves the unit unobserved, which is the dead case.
//! Successor edges move registers and storage, never flag units, so the
//! walk needs no transport audit. The compare itself may not be named by a
//! call contract or memory-access row.
//!
//! The sibling [`redundant`] family removes a compare whose publication is
//! observed but adds nothing: every published unit already carries the same
//! flag state from flag-equivalent shadow compares — the last flag event on
//! every path to the compare, resolved through the shared condition-state
//! walk across block boundaries — with the operand registers provably
//! unchanged between each shadow and the compare, so the removal leaves
//! every reader observing identical values.
//!
//! The [`equivalent`] family relaxes the strict family's form identity to
//! value identity: the shadow may carry a different compare kind and read
//! different operand registers when each side of the subtraction provably
//! coincides — by shared register identity, audited stable along every
//! path, or by the literal a unique `MaterializeI64` producer pins
//! function-wide. It is the removal the `literal_compare` fold leaves
//! reachable: an immediate- or zero-form shadow against a later
//! register-form compare computes one subtraction, and the flag
//! publication, not the instruction kind, is what the reader observes.
//!
//! Removing the compare shortens its block's instruction vector, so
//! boundary settlements positioned after it shift one ordinal earlier;
//! positions at or before it are untouched, and the compare's register
//! uses simply disappear — producers whose outputs nobody reads stay for
//! the producer-elimination rules.
//!
//! Proposal and independent replay share only the admission predicates and
//! the settlement remap. Validation consumes the proposed program, requires
//! the function to equal the independently computed removal, and restores
//! the complete source by content — every other instruction, register,
//! roster row, call, and settlement included.

mod admission;
mod equivalent;
mod redundant;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use equivalent::{
    EquivalentCompareError, EquivalentCompareReceipt, ValidatedEquivalentCompare,
    remove_equivalent_compare, validate_equivalent_compare,
};
pub use redundant::{
    RedundantCompareError, RedundantCompareReceipt, ValidatedRedundantCompare,
    remove_redundant_compare, validate_redundant_compare,
};
pub use rewrite::remove_dead_compare;
pub use validation::validate_dead_compare;

#[cfg(test)]
mod tests;

/// An accepted dead-compare removal with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDeadCompare {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: DeadCompareReceipt,
}

impl ValidatedDeadCompare {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &DeadCompareReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadCompareReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl DeadCompareReceipt {
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
pub enum DeadCompareError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for DeadCompareError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid dead-compare removal: {self:?}")
    }
}

impl std::error::Error for DeadCompareError {}
