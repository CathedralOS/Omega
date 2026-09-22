//! Instruction relocation across one edge on the selected CFG.
//!
//! The in-block scheduling families — `rewrites/local_schedule`,
//! `rewrites/local_relocation`, and `rewrites/run_relocation` — prove
//! bounded moves whose window sits inside one block's body. This family is
//! the first move whose window crosses a block boundary: the named `member`
//! leaves its own block's body and takes the named `destination`
//! instruction's position in the successor block. The destination keeps its
//! own position's contents one slot later, so the member lands exactly
//! where the destination sat and every position between the member's
//! vacated index and the landing index keeps its relative order.
//!
//! The move is admitted only on the one edge shape whose two sides always
//! execute together: the member's block must end in an unconditional
//! `Jump` naming the destination's block, and that block must be a plain
//! `Source` block — never an edge-transfer bridge or a case-dispatch
//! continuation — reached by that edge alone. A conditional terminator
//! gives the member's block a second exit the member would still execute
//! on after relocating; a second predecessor gives the destination block a
//! path the member would newly execute on; the entry block is reached with
//! no predecessor at all. Between the member's old position and its new
//! one lie only the crossed positions the window audits: the member's own
//! block tail behind it, the `Jump` terminator instruction, the edge's
//! register transports, and the destination block's body instructions
//! before the landing index. Positions before the member's index, at or
//! after the landing index, and in every other block keep the member on
//! the same side they always had, so they are never crossed.
//!
//! The hazard audit is the in-block relocation's applied across the
//! boundary: a register or condition-state unit the member writes and a
//! crossed instruction reads or writes, or the member reads and a crossed
//! instruction writes, refuses in either direction. The edge's register
//! transports join the audit directly rather than through an instruction:
//! a `Registers` transport writes its parameter at the boundary, so a
//! member defining the transported argument, defining the parameter, or
//! reading the parameter would observe or feed a different value after the
//! move and refuses. Calls, hosted effects, terminator kinds, and
//! call-roster entries are barriers anywhere in the window — the `Jump`
//! terminator itself is the crossed edge, not a window member — and a
//! boundary settlement refuses exactly where an in-block window would
//! place one observing a changed prefix: past the member's index in its
//! own block, or past the landing index in the destination block.
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

pub use rewrite::relocate_selected_instruction_across_edge;
pub use validation::validate_edge_relocation;

#[cfg(test)]
mod tests;

/// An accepted cross-edge relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedEdgeRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: EdgeRelocationReceipt,
}

impl ValidatedEdgeRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &EdgeRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl EdgeRelocationReceipt {
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
pub enum EdgeRelocationError {
    SourceMismatch,
    /// A barrier kind, a call-roster entry, or an unaccounted
    /// memory-capable kind sits at a position the move would cross — the
    /// named member included.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible cross-edge window: the
    /// member's block lacks an unconditional `Jump` to the destination's
    /// block, the edge is not a plain semantic successor or carries case,
    /// fuel, or structural transfers, the destination block is the entry
    /// block, a non-source implementation block, a self-edge, or reached
    /// by a second predecessor, the destination names no body or
    /// terminator instruction there, a register or condition-state hazard
    /// couples the member with a crossed position or the edge's
    /// transports, a second memory-access actor shares the window, or a
    /// boundary settlement observes a changed executed prefix.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for EdgeRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid cross-edge relocation: {self:?}")
    }
}

impl std::error::Error for EdgeRelocationError {}
