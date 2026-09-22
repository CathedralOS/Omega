//! Instruction relocation upstream across one edge on the selected CFG.
//!
//! The cross-boundary families landed so far move a member either
//! downstream — `rewrites/edge_relocation` crosses a `Jump` edge into the
//! sole-predecessor target, `rewrites/diamond_relocation` sinks a member
//! out of the branching head through both arms into the one join they
//! feed, `rewrites/fork_relocation` sinks a member into the one arm the
//! named edge selects — or upstream through a complete branch diamond in
//! `rewrites/join_relocation`. This family is the upstream counterpart of
//! the single-edge move: the named `member` leaves its own block's body,
//! crosses the sole edge reaching that block, and takes the named
//! `destination` instruction's position inside the predecessor block that
//! edge leaves. The destination and every later position keep their
//! relative order one slot later, so the member lands exactly where the
//! destination sat.
//!
//! The move is admitted only on the one edge shape whose two sides always
//! execute together: the member's block must be reached by exactly one
//! edge, and that edge must be the unconditional `Jump` the destination
//! block ends in. A conditional predecessor keeps a second exit the member
//! would newly execute on after relocating; a second edge into the
//! member's block gives it a path the member would stop executing on; a
//! self-edge is the in-block family's case with a back-edge transport
//! reading. Between the member's old position and its new one lie only the
//! crossed positions the window audits: the destination block's body
//! instructions at and after the landing index, its `Jump` terminator
//! instruction, the edge's register transports, and the member's own
//! block's body instructions before its index. Positions before the
//! landing index, at or after the member's index, and in every other
//! block keep the member on the same side they always had, so they are
//! never crossed.
//!
//! The hazard audit is the in-block relocation's applied across the
//! boundary in the upstream direction: a register or condition-state unit
//! the member writes and a crossed instruction reads or writes, or the
//! member reads and a crossed instruction writes, refuses in either
//! direction. The edge's register transports join the audit directly
//! rather than through an instruction: a `Registers` transport reads its
//! argument and writes its parameter at the boundary, so a member
//! defining the transported argument would hand the binding a new value
//! where the source bound the old, a member defining the parameter would
//! be overwritten before the member's block observes it, and a member
//! reading the parameter would observe the pre-transport value after the
//! move. Calls, hosted effects, terminator kinds, and call-roster entries
//! are barriers anywhere in the window — the `Jump` terminator itself is
//! the crossed edge, not a window member — and a boundary settlement
//! refuses exactly where an in-block window would place one observing a
//! changed prefix: past the member's index in its own block, or past the
//! landing index in the destination block.
//!
//! Memory ordering keeps the validated `memory_accesses` roster's
//! completeness discipline: a memory-capable member or crossed instruction
//! without rows is unaccounted and refuses, a roster-carrying member may
//! cross only row-less positions — the terminator instruction and any
//! rows the crossed edge itself records included — while a row-less member
//! crosses any accounted mix because no recorded access changes order.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: the member keeps its own id, kind, operands,
//! provenance, and implicit surface while only its position in the program
//! moves. Proposal and independent replay share only the admission
//! predicates. Validation consumes the proposed program, requires the
//! member to sit at the landing index, and restores the complete source
//! by content — every other block, instruction, register, roster row,
//! call, and settlement is retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_instruction_into_predecessor;
pub use validation::validate_predecessor_relocation;

#[cfg(test)]
mod tests;

/// An accepted relocation into the sole predecessor with its replay
/// receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPredecessorRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: PredecessorRelocationReceipt,
}

impl ValidatedPredecessorRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &PredecessorRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredecessorRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl PredecessorRelocationReceipt {
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
pub enum PredecessorRelocationError {
    SourceMismatch,
    /// A barrier kind, a call-roster entry, or an unaccounted
    /// memory-capable kind sits at a position the move would cross — the
    /// named member included.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible window into the
    /// predecessor: the member's block is the entry block or is not
    /// reached by exactly one edge, the sole edge into it is not a plain
    /// semantic successor of an unconditional `Jump`, the edge carries
    /// case, fuel, or structural transfers, the destination block is the
    /// member's own block or a non-source implementation block, the
    /// destination names no body or terminator instruction there, a
    /// register or condition-state hazard couples the member with a
    /// crossed position or the edge's transports, a second memory-access
    /// actor shares the window, or a boundary settlement observes a
    /// changed executed prefix.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for PredecessorRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid predecessor relocation: {self:?}")
    }
}

impl std::error::Error for PredecessorRelocationError {}
