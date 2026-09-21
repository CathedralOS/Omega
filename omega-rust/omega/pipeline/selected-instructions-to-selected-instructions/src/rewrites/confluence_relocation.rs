//! Instruction relocation into a confluence on the selected CFG.
//!
//! The landed downstream families move a member across one unconditional
//! `Jump` edge into the edge's sole-predecessor target
//! (`rewrites/edge_relocation`), out of a branching head through the arms
//! into the one join they alone feed (`rewrites/diamond_relocation` and
//! `rewrites/bypass_relocation`), or into the one arm a branch edge
//! selects (`rewrites/fork_relocation`). This family crosses the
//! remaining converging shape the single-edge move refuses: the named
//! `member` leaves its own block's body, whose terminator is a lone
//! `Jump` to a join at least one other predecessor's edge also reaches,
//! and takes the named `destination` instruction's position in that join
//! block. The destination and every later position keep their relative
//! order one slot later, so the member lands exactly where the
//! destination sat and every position the move crosses keeps its
//! relative order.
//!
//! Sinking into a confluence flips the soundness burden the
//! sole-predecessor move carries. `edge_relocation` needs no reach
//! audit: every traversal of its target arrives through the member's
//! block, so the relocated write is the same definition every path
//! always observed. Here the join's other inflows hand the member's new
//! position traversals it never executed on — speculation downstream —
//! so the member must be pure register and condition-state work that can
//! never fault: no barrier kind, no call-roster entry, no memory roster
//! rows, and no memory-capable or potentially-faulting kind, because an
//! access or trap that ran only on this inflow would newly run on every
//! arrival. And every register and condition-state unit the member
//! writes must be dead — unread until rewritten — from the landing index
//! forward: the join's tail, its terminator, and every reachable
//! successor position are shared by all inflows, so a reader there
//! observes the member's foreign definition on an arrival that never ran
//! it. The forward dead-path audit walks the join from the landing
//! index, where the member republishes its locations on every arrival,
//! and propagates them through the successor region — reading each
//! crossed edge's register, structural-binding, and case-payload
//! transports as boundary positions — refusing the moment a still-live
//! member definition meets a reader. On this inflow's own path the
//! relocated write is the familiar one the source produced, but the
//! shared region cannot tell the arrivals apart, so the audit holds the
//! die-unread rule on every continuation.
//!
//! The source side needs no region structure beyond the lone exit: the
//! member's block is any block ending in a plain `Jump` to the join — a
//! conditional terminator keeps a second exit the member would still
//! execute on, which is the fork, diamond, and triangle families'
//! shapes. The join needs only the in-edge count the single-edge family
//! excludes — at least one edge leaving another block — and the usual
//! boundary honesty: not the member's own block, which is the in-block
//! family's case with a back-edge reading, not the entry block, which is
//! reached with no predecessor at all, and a plain source block, because
//! an implementation block's origin carries edge or case work the
//! bounded audit does not cross. The crossed edge itself must be a plain
//! semantic successor: case custody, fuel, and structural transfers are
//! boundary effects the move does not cross.
//!
//! Between the member's old position and its new one lie only the
//! crossed positions the window audits: the member's own block tail
//! behind it, the `Jump` terminator instruction, the edge's register
//! transports, and the join's body instructions before the landing
//! index. Positions before the member's index, at or after the landing
//! index, and in every other block keep the member on the side they
//! always had, so they are never crossed. The join's other inflow edges
//! are never traversed by the member's old position — they run before
//! the landing index on their own arrivals — so their transports join
//! only the dead-path audit's boundary reading where loops carry the
//! foreign set back around.
//!
//! The hazard audit is the shared run relocation's applied across the
//! boundary downstream — `block_edges::crossed_window` derives the lone
//! `Jump` edge's positions and `window_hazards::admit_run_relocation`
//! proves the window independent once: a register or condition-state
//! unit the member writes and a crossed instruction reads or writes, or
//! the member reads and a crossed instruction writes, refuses in either
//! direction — which
//! also pins the member's inputs, so the execution at the landing index
//! on this inflow's path computes the values the old position computed.
//! The edge's register transports join the audit directly rather than
//! through an instruction: a `Registers` transport reads its argument
//! and writes its parameter at the boundary, so a member defining the
//! transported argument would hand the binding the pre-move value where
//! the source bound the member's own write, a member defining the
//! parameter would be overwritten before the join observes it, and a
//! member reading the parameter would observe the transported value only
//! after the move. Calls, hosted effects, terminator kinds, and
//! call-roster entries are barriers anywhere in the window — the `Jump`
//! terminator itself is the crossed edge's position, not a window
//! member — and a boundary settlement refuses exactly where an executed
//! prefix changes: past the member's index in its own block, or past the
//! landing index in the join block.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: the member keeps its own id, kind,
//! operands, provenance, and implicit surface while only its position in
//! the program moves. Proposal and independent replay share only the
//! window primitives — `crossed_window`, the run audit, and the
//! dead-path walk — while each derives the family's legality decision
//! itself. Validation consumes the proposed program, re-derives the
//! member, join, and landing index from the source without the
//! producer's admission routine, requires the member to sit at the
//! landing index, and restores the complete source by content — every
//! other block, instruction, register, roster row, call, and settlement
//! is retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_instruction_into_confluence;
pub use validation::validate_confluence_relocation;

#[cfg(test)]
mod tests;

/// An accepted relocation into the confluence with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedConfluenceRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ConfluenceRelocationReceipt,
}

impl ValidatedConfluenceRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &ConfluenceRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluenceRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl ConfluenceRelocationReceipt {
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
pub enum ConfluenceRelocationError {
    SourceMismatch,
    /// The named member can never speculate past a confluence: a barrier
    /// kind, a call-roster entry, a roster-carrying or unaccounted
    /// memory-capable kind — an access or fault that ran only on this
    /// inflow would newly run on every arrival — or a barrier kind,
    /// call, or unaccounted memory-capable instruction at a position the
    /// move would cross.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible confluence window:
    /// the member's block lacks the lone unconditional `Jump`, or its
    /// edge is not a plain semantic successor or carries case, fuel, or
    /// structural transfers; the join is the member's own block, the
    /// entry block, a non-source block, or reached by no edge leaving
    /// another block — the sole-predecessor shape is the single-edge
    /// family's; the destination names no body or terminator instruction
    /// there; a register or condition-state hazard couples the member
    /// with a crossed position or the crossed edge's transports; or the
    /// dead-path audit finds a register or unit the member writes still
    /// live at a reader on a path the member never executed on before.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ConfluenceRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid confluence relocation: {self:?}")
    }
}

impl std::error::Error for ConfluenceRelocationError {}
