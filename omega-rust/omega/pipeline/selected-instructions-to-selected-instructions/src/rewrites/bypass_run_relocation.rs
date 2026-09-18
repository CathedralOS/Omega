//! Run relocation through a bypassed branch arm on the selected CFG.
//!
//! The in-block `rewrites/run_relocation` moves the contiguous run two
//! named members bound inside one body block, the cross-boundary
//! `rewrites/edge_run_relocation` carries that run across one lone
//! `Jump` edge into its sole-predecessor target, and
//! `rewrites/diamond_run_relocation` moves the run through the complete
//! branch diamond whose arms alone feed the join. This family is the
//! run move's window carried across the remaining converging shape —
//! the triangle: the named `first_member` and `last_member` bound one
//! contiguous run of at least two members in the branching block's
//! body, whose terminator names the join directly on at least one edge
//! — the bypass — while every other distinct target is an arm ending
//! in a `Jump` back to it. The run leaves that body as a single body —
//! its members keeping their own order — and lands on the named
//! `destination` instruction's position in the join. The destination
//! and every later position keep their relative order one run-width
//! later, so the run lands exactly where the destination sat and every
//! position the move crosses keeps its relative order.
//!
//! Moving the run as one body admits the window the member move refuses
//! on internal coupling alone: a producer whose only crossed reader is
//! the run's own next member cannot leave its block — the member move
//! would starve the consumer it leaves behind — while the run carries
//! the consumer with it, so internally coupled members cross the
//! triangle together and only positions outside the run count as
//! crossed.
//!
//! The move is admitted only on the one multi-edge shape whose sides
//! still always execute together once: at least one branch edge lands
//! on the join directly — the all-arms shape is the diamond family's —
//! every other distinct target must be a plain `Source` block reached
//! by that branch's edges alone and must end in an unconditional `Jump`
//! to the join, and every edge into the join must leave the head or an
//! arm. Then each traversal of the run's block reaches the join exactly
//! once, whether it bypassed the arm or ran it, so relocating the run
//! keeps its execution count at one per traversal of its original
//! block. A second predecessor into an arm would give the join a path
//! the run never executed on; a predecessor into the join leaving
//! anywhere else — including the join's own self-loop — would do the
//! same directly; a conditional exit or non-source origin inside an arm
//! would hand the move an edge-transfer or case-dispatch boundary this
//! step does not cross.
//!
//! Between the run's old span and its new one lie only the crossed
//! positions the window audits: the run's own block tail behind it, the
//! branch terminator instruction, both branch edges' register
//! transports — the bypass edge's included — every arm's complete body
//! and `Jump` terminator with its outgoing transports, and the join
//! block's body instructions before the landing index. Positions before
//! the run, at or after the landing index, and in every other block
//! keep the run on the side they always had, so they are never crossed.
//!
//! The hazard audit is the in-block run relocation's applied across the
//! triangle: a register or condition-state unit any member writes and a
//! crossed instruction reads or writes, or any member reads and a
//! crossed instruction writes, refuses in either direction. Every
//! crossed edge's register transports join the audit per member rather
//! than through an instruction: a `Registers` transport writes its
//! parameter at the boundary, so a member defining the transported
//! argument, defining the parameter, or reading the parameter would
//! observe or feed a different value after the move and refuses. Calls,
//! hosted effects, terminator kinds, and call-roster entries are
//! barriers anywhere in the window — the branch terminator and the
//! arms' `Jump` terminators are the crossed edges, not window members —
//! and a boundary settlement refuses where the run changes sides with
//! its block's executed prefix: past the run's first index in its own
//! block, or past the landing index in the join block.
//!
//! Memory ordering keeps the validated `memory_accesses` roster's
//! completeness discipline at run granularity: a memory-capable member
//! or crossed instruction without rows is unaccounted and refuses, a
//! roster-carrying run may cross only row-less positions — each
//! terminator instruction and the rows each crossed edge records
//! included — while a row-less run crosses any accounted mix because no
//! recorded access changes order. Members inside the run keep their
//! recorded order against each other, so two roster-carrying members
//! may share the run.
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

pub use rewrite::relocate_selected_run_through_bypass;
pub use validation::validate_bypass_run_relocation;

#[cfg(test)]
mod tests;

/// An accepted cross-triangle run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedBypassRunRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: BypassRunRelocationReceipt,
}

impl ValidatedBypassRunRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &BypassRunRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BypassRunRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl BypassRunRelocationReceipt {
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
pub enum BypassRunRelocationError {
    SourceMismatch,
    /// A barrier kind, a call-roster entry, or an unaccounted
    /// memory-capable kind sits at a position the move would cross — a
    /// run member included.
    UnsupportedInstruction,
    /// The named triple does not bound an admissible cross-triangle run
    /// window: the named members bound no contiguous run of at least two
    /// members in one block's body, the run's block does not end in a
    /// two-successor conditional branch on plain semantic edges, no
    /// branch edge lands on the join directly — the all-arms shape is
    /// the diamond family's — a branch or arm edge is not a plain
    /// semantic successor or carries case, fuel, or structural
    /// transfers, an arm is the run's own block, the entry block, a
    /// non-source implementation block, reached by a predecessor other
    /// than the branch, or fails to end in a plain `Jump` to the join,
    /// the join is the run's block, an arm, the entry block, a
    /// non-source block, or reached by an edge leaving neither the head
    /// nor an arm, the destination names no body or terminator
    /// instruction there, a register or condition-state hazard couples a
    /// member with a crossed position or a crossed edge's transports, a
    /// roster-carrying run shares the window with a second memory-access
    /// actor, or a boundary settlement observes a changed executed
    /// prefix.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for BypassRunRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid bypass run relocation: {self:?}")
    }
}

impl std::error::Error for BypassRunRelocationError {}
