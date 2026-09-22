//! Local scheduling relocation of a run under proven memory commutation on
//! the selected CFG.
//!
//! Scheduling here means changing the order instructions execute inside one
//! block without changing what any of them observes. The primitive this
//! family proves is the bounded run move with more than one accounted
//! memory actor: the named `first_member` and `last_member` bound one
//! contiguous run of at least two body instructions in their block, the
//! named `destination` instruction sits outside the run, and the run takes
//! the destination's position as one body — the crossed positions shifting
//! one run-width toward the run's vacated span in their original order.
//! That is exactly `rewrites/run_relocation`'s geometry, with one admission
//! rule widened: where the run relocation requires every position a
//! row-carrying run trades order with to be row-less, this family lets
//! accounted actors face each other when every pair of roster rows that
//! newly trades order commutes — the same access-independence audit the
//! commuting interchange families apply at the swap granularities and the
//! commuting member relocation applies at the single-member move, shared
//! through `rewrites/commuting_accesses`. A window whose trading pairs
//! carry at most one rowed side — a row-less run, or a run facing only
//! row-less positions — is the run relocation's own accounting case and
//! refuses here, keeping the two families disjoint; a one-member run is the
//! commuting relocation's granularity and refuses here for the same reason.
//!
//! Two recorded accesses commute when neither can observe the other's
//! effect on the bytes it reaches — two non-writing rows never conflict,
//! rows reaching provably distinct storage never conflict, and rows on
//! shared storage must be disjoint fixed extents. The run's members are
//! the only instructions whose position relative to the crossed run
//! changes, so the only row pairs that newly trade order are each member's
//! against each crossed position's; members keep their relative order
//! inside the run — internally coupled members move together through a
//! window neither could cross alone — crossed positions keep their
//! relative order with each other and with every position outside the
//! window, and positions before the window are untouched. Because every
//! traded row pair commutes, the relative order of conflicting accesses is
//! unchanged: none of the traded pairs conflict. The roster itself then
//! follows the new execution order — the rows naming the window's
//! instructions are permuted to match, each instruction's own rows keeping
//! their relative order — so the recorded accesses still appear in the
//! order the program performs them.
//!
//! Register and condition-state hazards, barriers, and settlements keep
//! the run relocation's audit unchanged: a register or unit a member
//! writes may not be read or written by a crossed position in either
//! direction, and one a member reads may not be written there — calls,
//! hosted effects, terminator kinds, and call-roster entries never sit in
//! the window, and a boundary settlement inside the window's span would
//! observe a different executed prefix. Since no location is written by
//! both a member and a crossed position, the last writer before every
//! later observer — the block's terminator and edge transports included —
//! is the same instruction it was before the move.
//!
//! No instruction, register, constraint key, call, settlement, or edge
//! record changes: every member keeps its own id, kind, operands,
//! provenance, and implicit surface while only the run's position in the
//! block vector moves, and each roster row keeps its own identity while
//! only its position in the access vector follows the move.
//!
//! Proposal and independent replay share only the admission predicates.
//! Validation consumes the proposed program, requires the touched block's
//! window and the roster's window rows to equal the independently computed
//! rotation, and restores the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_commuting_run;
pub use validation::validate_commuting_run_relocation;

#[cfg(test)]
mod tests;

/// An accepted commuting run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCommutingRunRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: CommutingRunRelocationReceipt,
}

impl ValidatedCommutingRunRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &CommutingRunRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommutingRunRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl CommutingRunRelocationReceipt {
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
pub enum CommutingRunRelocationError {
    SourceMismatch,
    /// A window position can never trade order: a barrier kind, a
    /// call-roster entry, or a memory-capable kind the roster does not
    /// account for — as a run member or as a crossed position.
    UnsupportedInstruction,
    /// The named instructions do not bound an admissible commuting
    /// window: absent or misordered in the run's block, a run of one
    /// member — the commuting relocation's granularity — a destination
    /// inside the run or outside the block, a member coupled by a
    /// register or condition-state hazard with a crossed position, a
    /// roster row that newly trades order not commuting with a crossed
    /// position's row, no rowed trading pair — the run relocation's own
    /// accounting case — or a boundary settlement inside the window's
    /// span.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for CommutingRunRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid commuting run relocation: {self:?}")
    }
}

impl std::error::Error for CommutingRunRelocationError {}
