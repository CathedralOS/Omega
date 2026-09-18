//! Local scheduling under proven memory commutation on the selected CFG.
//!
//! Scheduling here means changing the order instructions execute inside one
//! block without changing what any of them observes. The primitive this
//! family proves is the bounded pair interchange with more than one
//! accounted memory actor: two named body instructions exchange places —
//! the later scheduled ahead of the earlier — while every instruction
//! between them keeps its position. That is exactly
//! `rewrites/local_schedule`'s geometry, with one admission rule widened:
//! where the pair interchange requires at most one roster-carrying
//! position among the instructions that trade order, this family lets a
//! second accounted actor sit in the window when every pair of roster
//! rows that newly trades order commutes. A window whose trading pairs
//! carry at most one rowed side is the pair interchange's own case and
//! refuses here, keeping the two families disjoint.
//!
//! Two recorded accesses commute when neither can observe the other's
//! effect on the bytes it reaches. The roster's own overlap discipline —
//! the same `place` identity, slot storage, and byte-extent reasoning the
//! dead-store and store-motion walks apply — decides: two non-writing
//! rows (a place read, a dynamic-extent read, or a local or outgoing
//! address exposure) never conflict, since neither changes what the other
//! observes; rows reaching provably distinct storage — different places,
//! different local slots, different outgoing slots, or a place against a
//! slot that is not its storage or any outgoing slot — never conflict
//! either, because the roster's places and slot identities name disjoint
//! storage. Only a shared storage decides further: same place or same
//! slot, at least one write, and the two recorded byte ranges must be
//! disjoint fixed extents — a dynamic-extent span or sequence on the same
//! storage refuses, since its reach cannot be bounded away from the other
//! row. The recorded `place` on a slot row is the slot's own structural
//! place, so the slot check asks the place-storage question of the
//! *other* row's place, matching the established `interferes` reading.
//!
//! Because the traded rows commute, every access still observes the same
//! bytes and leaves the same bytes behind: the relative order of
//! conflicting accesses is unchanged because none of the traded pairs
//! conflict. The roster itself then follows the new execution order — the
//! rows naming the window's instructions are permuted to match, so the
//! recorded accesses still appear in the order the program performs them.
//! That permutation is the one content change beyond the instruction swap
//! the earlier scheduling families did not need; replay re-derives it
//! independently and restoring the source must reproduce the plan
//! bit-for-bit.
//!
//! Register and condition-state hazards, barriers, and settlements keep
//! the pair interchange's audit unchanged: no register or unit a member
//! writes may be read or written by a crossed position in either
//! direction, calls, hosted effects, and call-roster entries never sit in
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

mod accesses;
mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::interchange_selected_commuting_pair;
pub use validation::validate_commuting_interchange;

#[cfg(test)]
mod tests;

/// An accepted commuting pair interchange with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCommutingInterchange {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: CommutingInterchangeReceipt,
}

impl ValidatedCommutingInterchange {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &CommutingInterchangeReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommutingInterchangeReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl CommutingInterchangeReceipt {
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
pub enum CommutingInterchangeError {
    SourceMismatch,
    /// A window position can never trade order: a barrier kind, a
    /// call-roster entry, or a memory-capable kind the roster does not
    /// account for — as a named member or as a crossed interior position.
    UnsupportedInstruction,
    /// The named instructions do not bound an admissible commuting
    /// window: absent or misordered in the earlier member's block, coupled
    /// by a register or condition-state hazard with a crossed position,
    /// trading a roster row that does not commute with a crossed
    /// position's row, carrying no rowed-trading pair — the pair
    /// interchange's own accounting case — or separated by a boundary
    /// settlement inside the window's span.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for CommutingInterchangeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid commuting interchange: {self:?}")
    }
}

impl std::error::Error for CommutingInterchangeError {}
