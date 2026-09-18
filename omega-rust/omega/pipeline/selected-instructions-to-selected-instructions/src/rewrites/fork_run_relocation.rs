//! Run relocation into a fork arm on the selected CFG.
//!
//! The in-block `rewrites/run_relocation` moves the contiguous run two
//! named members bound inside one body block, and the downstream
//! cross-boundary families carry that run across the shapes whose every
//! traversal still executes it once — `rewrites/edge_run_relocation`,
//! `rewrites/predecessor_run_relocation`,
//! `rewrites/diamond_run_relocation`, `rewrites/bypass_run_relocation`,
//! and `rewrites/confluence_run_relocation`. This family is the run move
//! carried across the remaining branching shape the member move opened —
//! the fork: the named `first_member` and `last_member` bound one
//! contiguous run of at least two members in a block's body whose
//! terminator is a two-successor conditional branch, and the run leaves
//! that body as a single body — its members keeping their own order —
//! through the one plain branch edge the named `destination` selects,
//! taking that instruction's position in the landing arm's body. The
//! destination and every later position keep their relative order one
//! run-width later, so the run lands exactly where the destination sat
//! and every position the move crosses keeps its relative order.
//!
//! Moving the run as one body admits the window the member move refuses
//! on internal coupling alone: a producer whose only crossed reader is
//! the run's own next member cannot leave its block — the member move
//! would starve the consumer it leaves behind — while the run carries
//! the consumer with it, so internally coupled members cross the fork
//! together and only positions outside the run count as crossed.
//!
//! Sinking a run through one selected edge carries the member move's
//! conditional-execution burden at body granularity. The branch's other
//! edges hand the run's old position traversals it no longer executes
//! on, so every member must be pure register and condition-state work
//! that can never fault: no barrier kind, no call-roster entry, no
//! memory roster rows, and no memory-capable or potentially-faulting
//! kind, because an access or trap that ran on every traversal would run
//! only on the landing path after the move. And every register and
//! condition-state unit any member writes must be dead — unread until
//! rewritten — along every path leaving the branch's other edges: a
//! reader there observes the run's missing definitions where the source
//! still published them. The forward dead-path audit walks each skipped
//! edge's target region — reading each crossed edge's register,
//! structural-binding, and case-payload transports as boundary positions
//! — refusing the moment a still-live member definition meets a reader.
//! On the landing path the relocated writes are the familiar ones the
//! source produced; on every other path they simply never happen.
//!
//! The destination selects the landing arm: exactly one distinct branch
//! target may name it, and every edge into that arm must leave the
//! run's own block — a second predecessor would hand the arm's stream a
//! run that never ran on that path. Both branch edges may reach the one
//! arm, a degenerate fork where no traversal skips the run and the
//! dead-path audit is vacuous. The arm needs only the boundary honesty
//! the bounded audit requires: not the run's own block, which is the
//! in-block family's case with a back-edge reading, not the entry
//! block, which is reached with no predecessor at all, and a plain
//! source block, because an implementation block's origin carries edge
//! or case work the bounded audit does not cross. Each landing edge
//! itself must be a plain semantic successor: case custody, fuel, and
//! structural transfers are boundary effects the move does not cross —
//! the skipped edges' surfaces join only the dead-path audit's boundary
//! reading.
//!
//! Between the run's old span and its new one lie only the crossed
//! positions the window audits: the run's own block tail behind it, the
//! branch terminator instruction, the landing edge's register
//! transports, and the arm's body instructions before the landing
//! index. Positions before the run, at or after the landing index, and
//! in every other block keep the run on the side they always had, so
//! they are never crossed. The skipped edges and everything past them
//! are never traversed by the run's new position — so their blocks join
//! only the dead-path audit, where the arm's own terminator and body
//! sit inside the walked region when a loop carries the live set back
//! around.
//!
//! The hazard audit is the member move's applied across the boundary at
//! run granularity: a register or condition-state unit any member writes
//! and a crossed instruction reads or writes, or any member reads and a
//! crossed instruction writes, refuses in either direction — which also
//! pins each member's inputs, so the run's execution at the landing
//! index on the landing path computes the values the old positions
//! computed. The landing edge's register transports join the audit per
//! member rather than through an instruction: a `Registers` transport
//! reads its argument and writes its parameter at the boundary, so a
//! member defining the transported argument would hand the binding the
//! pre-move value where the source bound the member's own write, a
//! member defining the parameter would be overwritten before the arm
//! observes it, and a member reading the parameter would observe the
//! transported value only after the move. Calls, hosted effects,
//! terminator kinds, and call-roster entries are barriers anywhere in
//! the window — the branch terminator itself is the crossed edge's
//! position, not a window member — and a boundary settlement refuses
//! exactly where an executed prefix changes: past the run's first index
//! in its own block, or past the landing index in the arm.
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

pub use rewrite::relocate_selected_run_into_arm;
pub use validation::validate_fork_run_relocation;

#[cfg(test)]
mod tests;

/// An accepted cross-fork run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedForkRunRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ForkRunRelocationReceipt,
}

impl ValidatedForkRunRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &ForkRunRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkRunRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl ForkRunRelocationReceipt {
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
pub enum ForkRunRelocationError {
    SourceMismatch,
    /// A barrier kind, a call-roster entry, a roster-carrying or
    /// unaccounted memory-capable kind, or a potentially-faulting kind
    /// sits at a position the move would cross — a run member included:
    /// an access or trap that ran on every traversal would run only on
    /// the landing path after the move.
    UnsupportedInstruction,
    /// The named triple does not bound an admissible cross-fork run
    /// window: the named members bound no contiguous run of at least two
    /// members in one block's body, the run's block does not end in a
    /// two-successor conditional branch, the destination names no
    /// position in exactly one distinct branch target, the arm is the
    /// run's block, the entry block, a non-source block, or reached by
    /// an edge leaving another block, a landing edge is not a plain
    /// semantic successor, a register or condition-state hazard couples
    /// a member with a crossed position or a landing edge's transports,
    /// a boundary settlement observes a changed executed prefix, or the
    /// dead-path audit finds a register or unit a member writes still
    /// live at a reader on a path the run no longer executes on.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ForkRunRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid fork run relocation: {self:?}")
    }
}

impl std::error::Error for ForkRunRelocationError {}
