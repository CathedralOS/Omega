//! Run relocation across one edge on the selected CFG.
//!
//! The in-block `rewrites/run_relocation` moves the contiguous run two
//! named members bound inside one body block, and the cross-boundary
//! `rewrites/edge_relocation` moves one member across its block's lone
//! `Jump` edge into the edge's sole-predecessor target. This family is the
//! run move's window carried across that edge: the named `first_member`
//! and `last_member` bound one contiguous run of at least two members in
//! their block's body, the run leaves that body as a single body — its
//! members keeping their own order — and lands on the named
//! `destination` instruction's position in the successor block. The
//! destination and every later position keep their relative order one
//! run-width later, so the run lands exactly where the destination sat
//! and every position the move crosses keeps its relative order.
//!
//! Moving the run as one body admits the window the member move refuses
//! on internal coupling alone: a producer whose only crossed reader is
//! the run's own next member cannot leave its block — the member move
//! would starve the consumer it leaves behind — while the run carries
//! the consumer with it, so internally coupled members cross the
//! boundary together and only positions outside the run count as
//! crossed.
//!
//! The move is admitted only on the one edge shape whose two sides
//! always execute together: the run's block must end in an unconditional
//! `Jump` naming the destination's block, and that block must be a plain
//! `Source` block — never an edge-transfer bridge or a case-dispatch
//! continuation — reached by that edge alone. A conditional terminator
//! gives the run's block a second exit the run would still execute on
//! after relocating; a second predecessor gives the destination block a
//! path the run would newly execute on; the entry block is reached with
//! no predecessor at all. Between the run's old span and its new one lie
//! only the crossed positions the window audits: the run's own block
//! tail behind it, the `Jump` terminator instruction, the edge's
//! register transports, and the destination block's body instructions
//! before the landing index. Positions before the run, at or after the
//! landing index, and in every other block keep the run on the same side
//! they always had, so they are never crossed.
//!
//! The hazard audit is the in-block run relocation's applied across the
//! boundary: a register or condition-state unit any member writes and a
//! crossed instruction reads or writes, or any member reads and a
//! crossed instruction writes, refuses in either direction. The edge's
//! register transports join the audit per member rather than through an
//! instruction: a `Registers` transport writes its parameter at the
//! boundary, so a member defining the transported argument, defining the
//! parameter, or reading the parameter would observe or feed a different
//! value after the move and refuses. Calls, hosted effects, terminator
//! kinds, and call-roster entries are barriers anywhere in the window —
//! the `Jump` terminator itself is the crossed edge's position, not a
//! window member — and a boundary settlement refuses exactly where an
//! executed prefix changes: past the run's first index in its own block,
//! or past the landing index in the destination block.
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

pub use rewrite::relocate_selected_run_across_edge;
pub use validation::validate_edge_run_relocation;

#[cfg(test)]
mod tests;

/// An accepted cross-edge run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedEdgeRunRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: EdgeRunRelocationReceipt,
}

impl ValidatedEdgeRunRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &EdgeRunRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeRunRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl EdgeRunRelocationReceipt {
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
pub enum EdgeRunRelocationError {
    SourceMismatch,
    /// A barrier kind, a call-roster entry, or an unaccounted
    /// memory-capable kind sits at a position the move would cross — a
    /// run member included.
    UnsupportedInstruction,
    /// The named triple does not bound an admissible cross-edge run
    /// window: the named members bound no contiguous run of at least two
    /// members in one block's body, the run's block lacks an
    /// unconditional `Jump` to the destination's block, the edge is not
    /// a plain semantic successor or carries case, fuel, or structural
    /// transfers, the destination block is the entry block, a non-source
    /// implementation block, a self-edge, or reached by a second
    /// predecessor, the destination names no body or terminator
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

impl std::fmt::Display for EdgeRunRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid cross-edge run relocation: {self:?}")
    }
}

impl std::error::Error for EdgeRunRelocationError {}
