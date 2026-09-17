//! Redundant compare removal on the selected CFG.
//!
//! A compare is redundant when the flag state it publishes is already the
//! state every path into it observes: an earlier compare in the same block
//! — the shadow — computed the same condition-state surface from the same
//! operand values, and nothing between them disturbed either. Unlike the
//! dead case the redundant compare's units may be live and read: readers
//! simply observe the shadow's identical publication, so the removal
//! changes no observable flag state.
//!
//! Flag equivalence is the strict form: the shadow must be a compare of the
//! same `SelectedInstructionKind` — immediate payload included — reading
//! the same operand registers in the same positions, and each unit the
//! redundant compare publishes must carry the same role at the shadow: a
//! defined unit must be among the shadow's implicit definitions, since the
//! same kind over equal operand bits republishes the same flags, and a
//! clobbered unit must be among the shadow's clobbers, since either
//! instruction leaves it unspecified. A mixed role would change what a
//! reader observes and refuses. A compare of a different kind is not
//! admitted even when it would compute the same subtraction — the literal
//! forms and the `CompareI64Zero` form keep distinct identities — and an
//! operand pattern that merely evaluates equal, like a swapped pair or a
//! materialized literal, is a different publication.
//!
//! Two audits carry the proof. First, every unit in the compare's implicit
//! definitions and clobbers must name the shadow as its last in-block flag
//! event: the backward scan to the shadow position must find it before any
//! other definition or clobber, which also rules out an intervening flag
//! event between the two compares. Second, every operand register the
//! compares share must hold the same value at both positions: the open
//! interval between the shadow and the compare may not write it — a
//! definition, a `UseDef`, or an early-clobbered operand all count — since
//! the same register could then name different values. Register reads in
//! the interval are harmless.
//!
//! The bound is deliberately narrow. The shadow must sit in the compare's
//! own block: a cross-block reaching compare needs the shared
//! condition-state walk over the predecessor cone plus an operand-stability
//! audit across every edge between the sites, edge transports included, and
//! is future work. And the compare's block may not lie on a cycle: a
//! traversal re-entering the block would observe the compare's own earlier
//! execution as the last flag event, an observation the single-shadow
//! equivalence cannot represent, so a block reachable from itself refuses.
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
