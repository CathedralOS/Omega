//! Instruction relocation out of a branch arm, upstream through the fork
//! edge that reaches it, on the selected CFG.
//!
//! The landed cross-boundary families move a member downstream across a
//! `Jump` edge (`rewrites/edge_relocation`), upstream across the sole
//! `Jump` edge reaching its block (`rewrites/predecessor_relocation`),
//! through a complete diamond (`rewrites/diamond_relocation` sinks,
//! `rewrites/join_relocation` rises), and into the one arm a fork edge
//! selects (`rewrites/fork_relocation`). This family is the upstream
//! counterpart of the fork move: the named `member` leaves a conditional
//! arm's body, crosses the head's plain edge into it, and takes the named
//! `destination` instruction's position inside the fork head's body. The
//! destination and every later position keep their relative order one
//! slot later, so the member lands exactly where the destination sat.
//!
//! Rising out of the arm flips the soundness burden the sink carries.
//! `fork_relocation` removes the member's execution from every traversal
//! leaving through the branch's other edges, so its written locations
//! must be dead there. This move *adds* the member's execution to those
//! traversals — speculation — so the member must be pure register and
//! condition-state work that can never fault: no barrier kind, no
//! call-roster entry, no memory roster rows, and no memory-capable or
//! potentially-faulting kind, because an access or trap that ran only on
//! the arm's path would newly run on every traversal of the head. And
//! every register and condition-state unit the member writes must be
//! dead — unread until rewritten — along every path leaving the head's
//! non-source edges: the forward dead-path audit walks the successor
//! region from each such edge, reading each crossed edge's register,
//! structural-binding, and case-payload transports as boundary positions,
//! and refuses the moment a still-live member definition meets a reader.
//! On those paths the member's own new position is transparent: its reads
//! belong to the added execution whose outputs must die anyway, and its
//! writes republish member locations that stay subject to the die-unread
//! rule — a re-executed member publishes values foreign to the path, not
//! the values a reader there observed before.
//!
//! The source side needs no region structure at all: the arm is any
//! source block whose every predecessor edge leaves the member's new
//! block, so each traversal reaching the arm is immediately preceded by
//! a head traversal that ran the member — the same count of one it had
//! inside the arm. A second predecessor into the arm would hand the arm's
//! stream a member that ran an extra time on that path, and a head
//! terminator that is not a two-successor conditional branch is the
//! single-edge family's case or no relocation at all. The crossed edges
//! must be plain semantic successors: case custody, fuel, and structural
//! transfers are boundary effects the move does not cross.
//!
//! Between the member's old position and its new one lie only the crossed
//! positions the window audits: the head's body instructions at and after
//! the landing index, the branch terminator instruction, each crossed
//! edge's register transports, and the arm's body instructions before the
//! member's index. Positions before the landing index, at or after the
//! member's index, and in every other block keep the member on the side
//! they always had, so they are never crossed. The non-source edges are
//! never traversed by the member's new position — a traversal takes them
//! after the member ran — so their transports join only the dead-path
//! audit, where an argument reading a still-live member definition
//! refuses and a parameter writing one retires it.
//!
//! The hazard audit is the in-block relocation's applied across the
//! boundary in the upstream direction: a register or condition-state unit
//! the member writes and a crossed instruction reads or writes, or the
//! member reads and a crossed instruction writes, refuses in either
//! direction — which also pins the member's inputs, so the execution at
//! the landing index computes the values the arm position computed. The
//! edge's register transports join the audit directly: a member defining
//! the transported argument would hand the binding a new value where the
//! source bound the old, a member defining the parameter would be
//! overwritten before the arm observes it, and a member reading the
//! parameter would observe the pre-transport value after the move. Calls,
//! hosted effects, terminator kinds, and call-roster entries are barriers
//! anywhere in the window — the branch terminator itself is the crossed
//! edges' position, not a window member — and a boundary settlement
//! refuses exactly where an executed prefix changes: past the member's
//! index in the arm, or past the landing index in the head.
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
mod dead_path;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_instruction_out_of_arm;
pub use validation::validate_arm_relocation;

#[cfg(test)]
mod tests;

/// An accepted relocation out of the branch arm with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedArmRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ArmRelocationReceipt,
}

impl ValidatedArmRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &ArmRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl ArmRelocationReceipt {
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
pub enum ArmRelocationError {
    SourceMismatch,
    /// The named member can never speculate: a barrier kind, a call-roster
    /// entry, a roster-carrying or unaccounted memory-capable kind — an
    /// access or fault that ran only on the arm's path would newly run on
    /// every head traversal — or a barrier kind, call, or unaccounted
    /// memory-capable instruction at a position the move would cross.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible arm window: the
    /// member's block is the entry block, a non-source implementation
    /// block, or reached by edges leaving more than one block — or by
    /// none; the head is the member's own block or a non-source block,
    /// or lacks the two-successor conditional terminator; the destination
    /// names no position in the head; a crossed edge is not a plain
    /// semantic successor or carries case, fuel, or structural transfers;
    /// a register or condition-state hazard couples the member with a
    /// crossed position or a crossed edge's transports; a boundary
    /// settlement observes a changed executed prefix; or the dead-path
    /// audit finds a register or unit the member writes still live at a
    /// reader on a path the member never executed on before.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ArmRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid arm relocation: {self:?}")
    }
}

impl std::error::Error for ArmRelocationError {}
