//! Run relocation through a converging branch diamond on the selected CFG.
//!
//! The in-block `rewrites/run_relocation` moves the contiguous run two
//! named members bound inside one body block, the cross-boundary
//! `rewrites/edge_run_relocation` carries that run across one lone
//! `Jump` edge into its sole-predecessor target, and
//! `rewrites/diamond_relocation` moves one member out of a branching
//! head through both arms into the one join they feed. This family is
//! the run move's window carried across the diamond: the named
//! `first_member` and `last_member` bound one contiguous run of at least
//! two members in the branching block's body, the run leaves that body
//! as a single body — its members keeping their own order — and lands on
//! the named `destination` instruction's position in the join block the
//! branch's arms alone feed. The destination and every later position
//! keep their relative order one run-width later, so the run lands
//! exactly where the destination sat and every position the move crosses
//! keeps its relative order.
//!
//! Moving the run as one body admits the window the member move refuses
//! on internal coupling alone: a producer whose only crossed reader is
//! the run's own next member cannot leave its block — the member move
//! would starve the consumer it leaves behind — while the run carries
//! the consumer with it, so internally coupled members cross the diamond
//! together and only positions outside the run count as crossed.
//!
//! The move is admitted only on the one multi-edge shape whose two sides
//! still always execute together once: every block the run's block
//! branches to — one distinct arm when both edges name the same block —
//! must be a plain `Source` block reached by that branch's edges alone
//! and must end in an unconditional `Jump` to one common join block, and
//! every edge into the join must leave an arm. Then each traversal of
//! the run's block executes exactly one arm and reaches the join exactly
//! once, so relocating the run keeps its execution count at one per
//! traversal of its original block. A second predecessor into an arm
//! would give the join a path the run never executed on; a second
//! predecessor into the join would do the same directly; a conditional
//! exit or non-source origin inside an arm would hand the move an
//! edge-transfer or case-dispatch boundary this step does not cross.
//!
//! Between the run's old span and its new one lie only the crossed
//! positions the window audits: the run's own block tail behind it, the
//! branch terminator instruction, both branch edges' register
//! transports, every arm's complete body and `Jump` terminator with its
//! outgoing transports, and the join block's body instructions before
//! the landing index. Positions before the run, at or after the landing
//! index, and in every other block keep the run on the side they always
//! had, so they are never crossed.
//!
//! The hazard audit is the shared run-relocation audit applied across
//! the diamond: `block_edges::crossed_window` derives the positions and
//! edges every acyclic path between the head and the join crosses — the
//! run's block tail, the branch terminator with its two edges, each
//! arm's whole body with its `Jump` edge, and the join head before the
//! landing index — and `window_hazards::admit_run_relocation` proves the
//! window independent. A register or condition-state unit any member
//! writes and a crossed instruction reads or writes, or any member
//! reads and a crossed instruction writes, refuses in either direction.
//! Every crossed edge's register transports join the audit per member
//! rather than through an instruction: a `Registers` transport writes
//! its parameter at the boundary, so a member defining the transported
//! argument, defining the parameter, or reading the parameter would
//! observe or feed a different value after the move and refuses. Calls,
//! hosted effects, terminator kinds, and call-roster entries are
//! barriers anywhere in the window — the branch terminator and the arms'
//! `Jump` terminators are the crossed edges, not window members — and a
//! boundary settlement refuses where the run changes sides with its
//! block's executed prefix: past the run's first index in its own
//! block, past the landing index in the join block, or anywhere inside
//! a crossed arm — every member runs before each arm point before the
//! move and after it once the run lands in the join.
//!
//! Memory ordering keeps the validated `memory_accesses` roster's
//! completeness discipline at run granularity: a memory-capable member
//! or crossed instruction without rows is unaccounted and refuses, a
//! roster-carrying run may cross only row-less positions — each
//! terminator instruction and the rows each crossed edge records
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

pub use rewrite::relocate_selected_run_through_diamond;
pub use validation::validate_diamond_run_relocation;

#[cfg(test)]
mod tests;

/// An accepted cross-diamond run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDiamondRunRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: DiamondRunRelocationReceipt,
}

impl ValidatedDiamondRunRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &DiamondRunRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiamondRunRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl DiamondRunRelocationReceipt {
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
pub enum DiamondRunRelocationError {
    SourceMismatch,
    /// A barrier kind, a call-roster entry, or an unaccounted
    /// memory-capable kind sits at a position the move would cross — a
    /// run member included.
    UnsupportedInstruction,
    /// The named triple does not bound an admissible cross-diamond run
    /// window: the named members bound no contiguous run of at least two
    /// members in one block's body, the run's block does not end in a
    /// two-successor conditional branch on plain semantic edges, an arm
    /// is not a plain source block the branch alone reaches or does not
    /// end in an unconditional `Jump` to the one common join, the join
    /// is the entry block, a non-source block, an arm, or the run's own
    /// block, a second predecessor reaches an arm or the join, the
    /// destination names no body or terminator instruction there, a
    /// register or condition-state hazard couples a member with a
    /// crossed position or a crossed edge's transports, a
    /// roster-carrying run shares the window with a second memory-access
    /// actor, or a boundary settlement observes a changed executed
    /// prefix.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for DiamondRunRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid diamond run relocation: {self:?}")
    }
}

impl std::error::Error for DiamondRunRelocationError {}
