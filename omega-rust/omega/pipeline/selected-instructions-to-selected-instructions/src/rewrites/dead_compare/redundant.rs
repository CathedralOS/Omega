//! Redundant compare removal on the selected CFG.
//!
//! A compare is redundant when the flag state it publishes is already the
//! state every path into it observes: earlier compares — the shadows —
//! computed the same condition-state surface from the same operand values,
//! and nothing between the last shadow on a path and the compare disturbed
//! either. Unlike the dead case the redundant compare's units may be live
//! and read: readers simply observe the shadow's identical publication, so
//! the removal changes no observable flag state.
//!
//! Flag equivalence is the strict form: a shadow must be a compare of the
//! same `SelectedInstructionKind` — immediate payload included — reading
//! the same operand registers in the same positions, and each unit the
//! redundant compare defines must be among the shadow's implicit
//! definitions, since the same kind over equal operand bits republishes
//! the same flags. A unit the compare only clobbers asks less: every
//! reaching event must leave it unspecified — a clobber, never a
//! definition — since either instruction leaves the unit unspecified. A
//! mixed role would change what a reader observes and refuses. A compare
//! of a different kind is not admitted even when it would compute the same
//! subtraction — the literal forms and the `CompareI64Zero` form keep
//! distinct identities — and an operand pattern that merely evaluates
//! equal, like a swapped pair or a materialized literal, is a different
//! publication.
//!
//! Two audits carry the proof. First, every unit in the compare's implicit
//! definitions and clobbers resolves through the shared condition-state
//! reaching walk — the last in-block event when one precedes the compare,
//! otherwise the least-fixpoint entry set over the predecessor cone — and
//! every site in the set must satisfy the unit's role: an equivalent
//! compare defining a defined unit, a clobber carrying a clobbered one.
//! The set form matters across edges: a diamond whose arms each republish
//! the same compare admits with two shadow sites, while an entry unknown,
//! an eventless path, or divergent reaching events all refuse. Second,
//! every operand register the compares share must hold the same value at
//! the compare as at the last shadow on that path: the audit walks
//! backward from the compare through every position that reaches it
//! without crossing a shadow site — body instructions, terminator-carried
//! instructions, and the parameter positions every crossed edge's
//! transports define — refusing any write to a shared register, since the
//! same register could then name a different value. Reads and writes a
//! later shadow re-publishes past are harmless.
//!
//! Cycles need no separate rule. A re-entering traversal's last flag event
//! is whatever the block's own last event for the unit was — often the
//! compare's own earlier execution, which is trivially flag-equivalent —
//! and the backward audit covers the loop positions the cyclic segment
//! crosses, so a block reachable from itself admits when both proofs hold
//! and refuses only on a genuine divergence.
//!
//! Removing the compare shortens its block's instruction vector, so
//! boundary settlements positioned after it shift one ordinal earlier —
//! the same remap the dead family applies — while positions at or before it
//! are untouched. The compare's register uses simply disappear.
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

pub use rewrite::remove_redundant_compare;
pub use validation::validate_redundant_compare;

#[cfg(test)]
mod tests;

/// An accepted redundant-compare removal with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRedundantCompare {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: RedundantCompareReceipt,
}

impl ValidatedRedundantCompare {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &RedundantCompareReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedundantCompareReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl RedundantCompareReceipt {
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
pub enum RedundantCompareError {
    SourceMismatch,
    UnsupportedInstruction,
    UnsupportedUse,
    ConstraintMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for RedundantCompareError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid redundant-compare removal: {self:?}")
    }
}

impl std::error::Error for RedundantCompareError {}
