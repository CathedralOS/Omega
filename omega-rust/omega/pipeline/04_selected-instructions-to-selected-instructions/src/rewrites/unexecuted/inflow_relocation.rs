//! Instruction relocation upstream onto one inflow of a confluence on the
//! selected CFG.
//!
//! The landed upstream families move a member out of its block across a
//! shape whose every traversal carries it: `rewrites/predecessor_relocation`
//! crosses the sole edge into a sole-predecessor block, and
//! `rewrites/arm_relocation`, `rewrites/join_relocation`, and
//! `rewrites/triangle_relocation` rise through a fork head whose edges
//! alone reach the member's region. The downstream `rewrites/confluence_relocation`
//! sinks a member through its block's lone `Jump` into a join at least one
//! other predecessor's edge also reaches, speculating the member onto
//! arrivals it never ran on. This family is that confluence move in the
//! upstream direction: the named `member` leaves its own block's body — a
//! block at least one other inflow edge also reaches — crosses the one
//! inflow edge the named `destination`'s block ends in, and takes the
//! destination's position inside that predecessor. The destination and
//! every later position keep their relative order one slot later, so the
//! member lands exactly where the destination sat and every position the
//! move crosses keeps its relative order.
//!
//! Hoisting onto one inflow flips the burden the downstream move carries.
//! `confluence_relocation` adds the member's execution to the join's other
//! arrivals; this move removes it: the member runs once per traversal of
//! the inflow block, whose lone `Jump` reaches the member's block exactly
//! once — so it still runs on every arrival through the crossed edge and
//! never again on an arrival through any other. No traversal gains an
//! execution, but a traversal that loses one must observe no difference:
//! the member must be pure register and condition-state work that cannot
//! fault, because a potentially-faulting kind or a memory access that ran
//! on every arrival would leave a trap or an access the other inflows'
//! continuations no longer perform. And every register and
//! condition-state unit the member writes must be dead — unread until
//! rewritten — along every path entering through the other inflows: the
//! member's write is missing where the source still ran it, so a reader
//! there meets a stale value where the source met the member's.
//!
//! The dead-path audit walks each other inflow edge into the member's
//! block, where the vacated index publishes the member's locations as
//! still-live — the divergence the missing write leaves behind — and
//! propagates them through the successor region, reading each crossed
//! edge's register, structural-binding, and case-payload transports as
//! boundary positions. A walked path that loops back through the inflow
//! block runs the member at its new position, where the landing is an
//! ordinary execution: the write republishes every location it carries
//! and clears the live set, matching what the source's member wrote on
//! the same traversal.
//!
//! The inflow side needs no region structure beyond the lone exit: the
//! destination's block is any plain source block ending in a `Jump` to
//! the member's block — a conditional terminator keeps a second exit the
//! member would newly execute on, which is the arm and join families'
//! burden, and a second edge out of the predecessor is the same shape.
//! The member's block needs only the inflow count the sole-predecessor
//! family excludes — at least one edge leaving another block — and the
//! usual boundary honesty: it may not be the entry block, whose first
//! traversal crosses no inflow edge at all, and the predecessor may not
//! be the member's own block, which is the in-block family's case with a
//! back-edge reading. The crossed edge itself must be a plain semantic
//! successor: case custody, fuel, and structural transfers are boundary
//! effects the move does not cross. The other inflow edges are never
//! crossed — whatever they carry runs before the member's old position on
//! their own arrivals — so they join only the dead-path audit's boundary
//! reading.
//!
//! Between the member's old position and its new one lie only the crossed
//! positions the window audits: the predecessor's body instructions at
//! and after the landing index, its `Jump` terminator instruction, the
//! edge's register transports, and the member's own block's body
//! instructions before its index. Positions after the member's index, at
//! or before the landing index, and in every other block keep the member
//! on the side they always had, so they are never crossed.
//!
//! The hazard audit is the in-block relocation's applied across the
//! boundary upstream: a register or condition-state unit the member
//! writes and a crossed instruction reads or writes, or the member reads
//! and a crossed instruction writes, refuses in either direction — which
//! also pins the member's inputs, so the execution at the landing index
//! on this inflow's path computes the values the old position computed.
//! The edge's register transports join the audit directly rather than
//! through an instruction: a `Registers` transport reads its argument and
//! writes its parameter at the boundary, so a member defining the
//! transported argument would hand the binding the pre-move value where
//! the source bound the member's own write, a member defining the
//! parameter would be overwritten before the join observes it, and a
//! member reading the parameter would observe the transported value only
//! after the move. Calls, hosted effects, terminator kinds, and
//! call-roster entries are barriers anywhere in the window — the `Jump`
//! terminator itself is the crossed edge's position, not a window
//! member — and a boundary settlement refuses exactly where an executed
//! prefix changes: past the member's index in its own block, or past the
//! landing index in the predecessor.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: the member keeps its own id, kind,
//! operands, provenance, and implicit surface while only its position in
//! the program moves. Proposal and independent replay share only the
//! admission predicates. Validation consumes the proposed program,
//! requires the member to sit at the landing index, and restores the
//! complete source by content — every other block, instruction,
//! register, roster row, call, and settlement is retained bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_instruction_onto_inflow;
pub use validation::validate_inflow_relocation;

#[cfg(test)]
mod tests;

/// An accepted relocation onto the inflow with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedInflowRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: InflowRelocationReceipt,
}

impl ValidatedInflowRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &InflowRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InflowRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl InflowRelocationReceipt {
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
pub enum InflowRelocationError {
    SourceMismatch,
    /// The named member can never shed its other-inflow executions: a
    /// barrier kind, a call-roster entry, a roster-carrying or unaccounted
    /// memory-capable kind, or a potentially-faulting kind — an access or
    /// trap that ran on every arrival would silently vanish from the
    /// continuations the move removes — or a barrier kind, call, or
    /// unaccounted memory-capable instruction at a position the move
    /// would cross.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible inflow window: the
    /// destination's block does not end in the lone unconditional `Jump`
    /// reaching the member's block, or its edge is not a plain semantic
    /// successor or carries case, fuel, or structural transfers; the
    /// member's block is the entry block, is reached by no edge leaving
    /// another block — the sole-inflow shape is the predecessor family's
    /// case — or is the destination's own block; the destination block is
    /// a non-source implementation block; the destination names no body
    /// or terminator instruction there; a register or condition-state
    /// hazard couples the member with a crossed position or the crossed
    /// edge's transports; or the dead-path audit finds a register or unit
    /// the member writes still live at a reader on an arrival the move
    /// removes.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for InflowRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid inflow relocation: {self:?}")
    }
}

impl std::error::Error for InflowRelocationError {}
