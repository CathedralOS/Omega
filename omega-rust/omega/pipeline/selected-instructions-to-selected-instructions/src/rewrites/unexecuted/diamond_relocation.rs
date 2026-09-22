//! Instruction relocation through a converging branch diamond on the
//! selected CFG.
//!
//! The narrower cross-boundary family — `rewrites/edge_relocation` —
//! proves a move whose window crosses one unconditional `Jump` edge into a
//! sole-predecessor target. This family is the first move whose window
//! crosses branching *and* converging control flow: the named `member`
//! leaves its own block's body, whose terminator is a two-successor
//! conditional branch, and takes the named `destination` instruction's
//! position in the join block the branch's arms alone feed. The
//! destination keeps its own position's contents one slot later, so the
//! member lands exactly where the destination sat and every position the
//! move crosses keeps its relative order.
//!
//! The move is admitted only on the one multi-edge shape whose two sides
//! still always execute together once: every block the member's block
//! branches to — one distinct arm when both edges name the same block —
//! must be a plain `Source` block reached by that branch's edges alone and
//! must end in an unconditional `Jump` to one common join block, and every
//! edge into the join must leave an arm. Then each traversal of the
//! member's block executes exactly one arm and reaches the join exactly
//! once, so relocating the member keeps its execution count at one per
//! traversal of its original block. A second predecessor into an arm
//! would give the join a path the member never executed on; a second
//! predecessor into the join would do the same directly; a conditional
//! exit or non-source origin inside an arm would hand the move an
//! edge-transfer or case-dispatch boundary this step does not cross.
//!
//! Between the member's old position and its new one lie only the crossed
//! positions the window audits: the member's own block tail behind it, the
//! branch terminator instruction, both branch edges' register transports,
//! every arm's complete body and `Jump` terminator with its outgoing
//! transports, and the join block's body instructions before the landing
//! index. Positions before the member's index, at or after the landing
//! index, and in every other block keep the member on the side they always
//! had, so they are never crossed.
//!
//! The hazard audit is the shared run-relocation audit applied across the
//! diamond: `block_edges::crossed_window` derives the positions and edges
//! every acyclic path between the head and the join crosses — the member's
//! block tail, the branch terminator with its two edges, each arm's whole
//! body with its `Jump` edge, and the join head before the landing index —
//! and `window_hazards::admit_run_relocation` proves the window
//! independent. A register or condition-state unit the member writes and a
//! crossed instruction reads or writes, or the member reads and a crossed
//! instruction writes, refuses in either direction. Every crossed edge's
//! register transports join the audit directly rather than through an
//! instruction: a `Registers` transport writes its parameter at the
//! boundary, so a member defining the transported argument, defining the
//! parameter, or reading the parameter would observe or feed a different
//! value after the move and refuses. Calls, hosted effects, terminator
//! kinds, and call-roster entries are barriers anywhere in the window —
//! the branch terminator and the arms' `Jump` terminators are the crossed
//! edges, not window members — and a boundary settlement refuses where the
//! member changes sides with its block's executed prefix: past the
//! member's index in its own block, past the landing index in the join
//! block, or anywhere inside a crossed arm — the member runs before every
//! arm point before the move and after it once it lands in the join.
//!
//! Memory ordering keeps the validated `memory_accesses` roster's
//! completeness discipline: a memory-capable member or crossed instruction
//! without rows is unaccounted and refuses, a roster-carrying member may
//! cross only row-less positions — each terminator instruction and the
//! rows each crossed edge records included — while a row-less member
//! crosses any accounted mix because no recorded access changes order.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: the member keeps its own id, kind, operands,
//! provenance, and implicit surface while only its position in the program
//! moves. Proposal and independent replay share only the admission
//! predicates. Validation consumes the proposed program, requires the
//! member to sit at the landing index, and restores the complete source by
//! content — every other block, instruction, register, roster row, call,
//! and settlement is retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_instruction_through_diamond;
pub use validation::validate_diamond_relocation;

#[cfg(test)]
mod tests;

/// An accepted diamond relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDiamondRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: DiamondRelocationReceipt,
}

impl ValidatedDiamondRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &DiamondRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiamondRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl DiamondRelocationReceipt {
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
pub enum DiamondRelocationError {
    SourceMismatch,
    /// A barrier kind, a call-roster entry, or an unaccounted
    /// memory-capable kind sits at a position the move would cross — the
    /// named member included.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible diamond window: the
    /// member's block lacks a two-successor conditional terminator, a
    /// branch or arm edge is not a plain semantic successor or carries
    /// case, fuel, or structural transfers, an arm is the member's own
    /// block, the entry block, a non-source implementation block, reached
    /// by a predecessor other than the branch, or fails to end in a plain
    /// `Jump` to the one join, the join is the member's block, an arm, the
    /// entry block, a non-source block, or reached by an edge outside the
    /// arms, the destination names no body or terminator instruction
    /// there, a register or condition-state hazard couples the member
    /// with a crossed position or a crossed edge's transports, a second
    /// memory-access actor shares the window, or a boundary settlement
    /// observes a changed executed prefix.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for DiamondRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid diamond relocation: {self:?}")
    }
}

impl std::error::Error for DiamondRelocationError {}
