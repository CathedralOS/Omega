//! Local scheduling of a member against a run under proven memory
//! commutation on the selected CFG.
//!
//! Scheduling here means changing the order instructions execute inside one
//! block without changing what any of them observes. The primitive this
//! family proves is the bounded member-against-run interchange with more
//! than one accounted memory actor: one named body instruction — the
//! `member` — and the contiguous run the named `run_first` and `run_last`
//! members bound, of at least two members, exchange places while every
//! instruction between them keeps its relative order, shifting by one less
//! than the run's length. That is exactly
//! `rewrites/member_run_interchange`'s geometry, with one admission rule
//! widened: where the member-against-run interchange requires every
//! position a row-carrying side trades order with to be row-less, this
//! family lets accounted actors face each other when every pair of roster
//! rows that newly trades order commutes — the same access-independence
//! audit `rewrites/commuting_interchange` applies at pair granularity and
//! `rewrites/commuting_run_interchange` at run granularity, shared through
//! `rewrites/commuting_accesses`. A window whose trading pairs carry at
//! most one rowed side is the member-against-run interchange's own
//! accounting case and refuses here, keeping the two families disjoint; a
//! one-member run is the commuting pair's granularity and refuses here for
//! the same reason.
//!
//! Two recorded accesses commute when neither can observe the other's
//! effect on the bytes it reaches — two non-writing rows never conflict,
//! rows reaching provably distinct storage never conflict, and rows on
//! shared storage must be disjoint fixed extents. Because every traded row
//! pair commutes, the relative order of conflicting accesses is unchanged:
//! none of the traded pairs conflict. The roster itself then follows the
//! new execution order — the rows naming the window's instructions are
//! permuted to match, each instruction's own rows keeping their relative
//! order — so the recorded accesses still appear in the order the program
//! performs them. Rows inside the run never trade order, since the run's
//! members keep their relative order as one body.
//!
//! Register and condition-state hazards, barriers, and settlements keep
//! the member-against-run interchange's audit unchanged: no register or
//! unit a member writes may be read or written by a crossed position in
//! either direction — the member's crossed positions are the run's members
//! and the whole interior, each run member's are the member and the whole
//! interior — calls, hosted effects, and call-roster entries never sit in
//! the window, and a boundary settlement inside the window's span would
//! observe a different executed prefix. Since no location is written by
//! both a member and a crossed position, the last writer before every
//! later observer — the block's terminator and edge transports included —
//! is the same instruction it was before the interchange, and positions
//! before the window are untouched.
//!
//! No instruction, register, constraint key, call, settlement, or edge
//! record changes: each instruction keeps its own id, kind, operands,
//! provenance, and implicit surface while only its position in the block
//! vector moves, and each roster row keeps its own identity while only
//! its position in the access vector follows the move.
//!
//! Proposal and independent replay share only the admission predicates.
//! Validation consumes the proposed program, requires the touched block's
//! window and the roster's window rows to equal the independently
//! computed interchange, and restores the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::interchange_selected_commuting_member_and_run;
pub use validation::validate_commuting_member_run_interchange;

#[cfg(test)]
mod tests;

/// An accepted commuting member-against-run interchange with its replay
/// receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCommutingMemberRunInterchange {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: CommutingMemberRunInterchangeReceipt,
}

impl ValidatedCommutingMemberRunInterchange {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &CommutingMemberRunInterchangeReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommutingMemberRunInterchangeReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl CommutingMemberRunInterchangeReceipt {
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
pub enum CommutingMemberRunInterchangeError {
    SourceMismatch,
    /// A window position can never trade order: a barrier kind, a
    /// call-roster entry, or a memory-capable kind the roster does not
    /// account for — as the member, as a run member, or as a crossed
    /// interior position.
    UnsupportedInstruction,
    /// The named instructions do not bound an admissible commuting
    /// window: absent or misordered bounds, a run of one member — the
    /// commuting pair's granularity — a member inside or equal to the
    /// run's span, a member or run member coupled by a register or
    /// condition-state hazard with a crossed position, a roster row that
    /// newly trades order not commuting with a crossed position's row,
    /// carrying no rowed-trading pair — the member-against-run
    /// interchange's own accounting case — or a boundary settlement
    /// inside the window's span.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for CommutingMemberRunInterchangeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid commuting member-against-run interchange: {self:?}"
        )
    }
}

impl std::error::Error for CommutingMemberRunInterchangeError {}
