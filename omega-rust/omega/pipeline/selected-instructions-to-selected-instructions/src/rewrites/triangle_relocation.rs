//! Instruction relocation out of a converging join, upstream through the
//! bypassed triangle that feeds it, on the selected CFG.
//!
//! The landed upstream families move a member across the sole `Jump` edge
//! reaching its block (`rewrites/predecessor_relocation`), out of a
//! converging join through the complete branch diamond whose arms alone
//! feed it (`rewrites/join_relocation`), or out of a conditional arm into
//! the fork head under a speculation proof
//! (`rewrites/arm_relocation`). This family rises through the remaining
//! converging shape — the triangle `rewrites/bypass_relocation` sinks
//! through: the named `member` leaves the join block's body, crosses the
//! head's own edge into the join, the arm and its `Jump`, the arm-ward
//! branch edge, and the branch itself, and takes the named `destination`
//! instruction's position in the fork head's body. The destination and
//! every later position keep their relative order one slot later, so the
//! member lands exactly where the destination sat.
//!
//! The soundness burden is the join family's total supply, unchanged by
//! the bypass edge. Rising out of the join keeps the member's execution
//! count at one per join traversal only when every path into the join
//! traversed the member's new position: every edge into the join leaves
//! the head directly — the bypass — or leaves an arm whose only
//! predecessors are the head's own edges and whose terminator is a plain
//! `Jump` back to the join, and every edge the head's two-successor
//! conditional terminator names lands on the join or an arm. Then each
//! traversal into the join passed the head exactly once, where the member
//! now runs, and each traversal of the head reaches the join exactly
//! once, where it ran before — so no fixpoint audit is needed. A second
//! predecessor into the join or an arm would hand the join's readers a
//! member that never ran on that path; a head edge leaving the region
//! would run the member on a traversal the join never saw; and a join fed
//! by the head on every edge — no arm at all — is the arm family's
//! shape, while a join fed by arms alone is the diamond family's. All
//! refuse.
//!
//! The member's value obligations are the window audit's: its reads must
//! observe the same values at the head's position they observed in the
//! join, so no crossed position — head suffix, branch terminator, either
//! branch edge's transports, the arm's body, the arm `Jump`, the arm's
//! join-ward edge's transports, or the join prefix behind the member —
//! may write a location it reads, and none may read or write a location
//! it writes. A transported register on any crossed edge binds the same
//! rule: a member defining the argument would hand the binding a new
//! value where the source bound the old, a member defining the parameter
//! would be overwritten before the join sees it, and a member reading the
//! parameter would observe the transported value only before the move.
//! The bypass edge's transports are crossed exactly like the arm-ward
//! edge's: the member ran behind both of them on their own traversals.
//!
//! Calls, hosted effects, terminator kinds, and call-roster entries are
//! barriers anywhere in the window — the branch and arm terminators are
//! crossed positions, not window members — and a boundary settlement
//! refuses where the member changes sides with its block's executed
//! prefix: past the member's index in the join, or past the landing index
//! in the head. The arm block needs no settlement audit: the member never
//! enters its body, so no arm prefix ever contained or loses it. Memory
//! motion stays the shared accounting: a roster-carrying member crosses
//! only row-less positions, each terminator and each edge's own access
//! rows counting as accounted boundary positions.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: the member keeps its own id, kind,
//! operands, provenance, and implicit surface while only its position in
//! the program moves. Proposal and independent replay share only the
//! admission predicates. Validation consumes the proposed program,
//! requires the member to sit at the landing index, and restores the
//! complete source by content — every other block, instruction, register,
//! roster row, call, and settlement is retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_instruction_out_of_triangle;
pub use validation::validate_triangle_relocation;

#[cfg(test)]
mod tests;

/// An accepted relocation out of the bypassed triangle's join with its
/// replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTriangleRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: TriangleRelocationReceipt,
}

impl ValidatedTriangleRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &TriangleRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriangleRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl TriangleRelocationReceipt {
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
pub enum TriangleRelocationError {
    SourceMismatch,
    /// The named member can never leave the converging side: a barrier
    /// kind, a call-roster entry, or an unaccounted memory-capable kind —
    /// or a barrier kind, call, or unaccounted memory-capable instruction
    /// at a position the move would cross.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible triangle window: the
    /// member's block is not a source-origin join, is the entry block, or
    /// is reached by an edge leaving a block that is neither the one fork
    /// head nor an arm it alone feeds — where an arm is a plain source
    /// block, not the join or the entry, ending in a plain `Jump` to the
    /// join — or by no direct head edge at all, which is the diamond
    /// family's shape, or by head edges alone, which is the arm family's;
    /// an arm is reached by a predecessor other than the one fork head;
    /// the head is the member's own block or a non-source block, or names
    /// an edge that lands on neither the join nor an arm; a crossed edge
    /// is not a plain semantic successor or carries case, fuel, or
    /// structural transfers; the destination names no position in the
    /// head; a register or condition-state hazard couples the member with
    /// a crossed position or a crossed edge's transports; a
    /// roster-carrying member shares the window with a second memory
    /// actor; or a boundary settlement observes a changed executed
    /// prefix.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for TriangleRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid triangle relocation: {self:?}")
    }
}

impl std::error::Error for TriangleRelocationError {}
