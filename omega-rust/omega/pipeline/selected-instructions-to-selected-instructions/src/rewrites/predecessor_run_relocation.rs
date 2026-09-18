//! Run relocation upstream across one edge on the selected CFG.
//!
//! The in-block `rewrites/run_relocation` moves the contiguous run two
//! named members bound inside one body block, and the upstream
//! `rewrites/predecessor_relocation` moves one member out of its block
//! across the sole edge reaching it into the predecessor that edge
//! leaves. This family is the run move's window carried across that
//! edge: the named `first_member` and `last_member` bound one contiguous
//! run of at least two members in their block's body, the run leaves
//! that body as a single body — its members keeping their own order —
//! and lands on the named `destination` instruction's position in the
//! sole-predecessor block. The destination and every later position keep
//! their relative order one run-width later, so the run lands exactly
//! where the destination sat and every position the move crosses keeps
//! its relative order.
//!
//! Moving the run as one body admits the window the member move refuses
//! on internal coupling alone: a member whose only crossed hazard meets
//! the run's own earlier member cannot leave its block — the member move
//! would cross the producer it reads — while the run carries the
//! producer with it, so internally coupled members cross the boundary
//! together and only positions outside the run count as crossed.
//!
//! The move is admitted only on the one edge shape whose two sides
//! always execute together: the run's block must be reached by exactly
//! one edge, and that edge must be the unconditional `Jump` the
//! destination block ends in. A conditional predecessor keeps a second
//! exit the run would newly execute on after relocating; a second edge
//! into the run's block gives it a path the run would stop executing on;
//! the entry block is reached with no predecessor at all. Between the
//! run's old span and its new one lie only the crossed positions the
//! window audits: the predecessor's body instructions at and after the
//! landing index, its `Jump` terminator instruction, the edge's register
//! transports, and the run's own block's body instructions before the
//! run's first index. Positions before the landing index, at or after
//! the run's last index, and in every other block keep the run on the
//! same side they always had, so they are never crossed.
//!
//! The hazard audit is the in-block run relocation's applied across the
//! boundary in the upstream direction: a register or condition-state
//! unit any member writes and a crossed instruction reads or writes, or
//! any member reads and a crossed instruction writes, refuses in either
//! direction. The edge's register transports join the audit per member
//! rather than through an instruction: a `Registers` transport reads its
//! argument and writes its parameter at the boundary, so a member
//! defining the transported argument would hand the binding a new value
//! where the source bound the old, a member defining the parameter would
//! be overwritten before the run's block observes it, and a member
//! reading the parameter would observe the pre-transport value after the
//! move. Calls, hosted effects, terminator kinds, and call-roster
//! entries are barriers anywhere in the window — the `Jump` terminator
//! itself is the crossed edge's position, not a window member — and a
//! boundary settlement refuses exactly where an executed prefix changes:
//! past the run's first index in its own block, or past the landing
//! index in the predecessor block.
//!
//! Memory ordering keeps the validated `memory_accesses` roster's
//! completeness discipline at run granularity: a memory-capable member
//! or crossed instruction without rows is unaccounted and refuses, a
//! roster-carrying run may cross only row-less positions — the
//! terminator instruction and any rows the crossed edge itself records
//! included — while a row-less run crosses any accounted mix because no
//! recorded access changes order. Members inside the run keep their
//! recorded order against each other, so two roster-carrying members may
//! share the run.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: every member keeps its own id, kind,
//! operands, provenance, and implicit surface while only the run's
//! position in the program moves. Proposal and independent replay share
//! only the admission predicates. Validation consumes the proposed
//! program, requires the run to sit at the landing index in its original
//! order, and restores the complete source by content — every other
//! block, instruction, register, roster row, call, and settlement is
//! retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_run_into_predecessor;
pub use validation::validate_predecessor_run_relocation;

#[cfg(test)]
mod tests;

/// An accepted cross-edge run relocation into the sole predecessor with
/// its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPredecessorRunRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: PredecessorRunRelocationReceipt,
}

impl ValidatedPredecessorRunRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &PredecessorRunRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredecessorRunRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl PredecessorRunRelocationReceipt {
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
pub enum PredecessorRunRelocationError {
    SourceMismatch,
    /// A barrier kind, a call-roster entry, or an unaccounted
    /// memory-capable kind sits at a position the move would cross — a
    /// run member included.
    UnsupportedInstruction,
    /// The named triple does not bound an admissible window into the
    /// predecessor: the named members bound no contiguous run of at
    /// least two members in one block's body, the run's block is the
    /// entry block or is not reached by exactly one edge, the sole edge
    /// into it is not a plain semantic successor of an unconditional
    /// `Jump`, the edge carries case, fuel, or structural transfers, the
    /// destination block is the run's own block or a non-source
    /// implementation block, the destination names no body or terminator
    /// instruction there, a register or condition-state hazard couples a
    /// member with a crossed position or the edge's transports, a
    /// roster-carrying run shares the window with a second memory-access
    /// actor, or a boundary settlement observes a changed executed
    /// prefix.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for PredecessorRunRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid predecessor run relocation: {self:?}")
    }
}

impl std::error::Error for PredecessorRunRelocationError {}
