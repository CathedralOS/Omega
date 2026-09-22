//! Instruction relocation through a diverging branch fork on the selected
//! CFG.
//!
//! The landed cross-boundary families — `rewrites/edge_relocation` and
//! `rewrites/diamond_relocation` — move a member across edges while keeping
//! its execution count at exactly one per traversal of its original block:
//! the single-edge move lands in a sole-predecessor target, and the diamond
//! move crosses every arm into the one join they alone feed. This family is
//! the first move whose landing makes the member's execution *conditional*:
//! the named `member` leaves its own block's body, whose terminator is a
//! two-successor conditional branch, crosses the branch's plain edge into
//! the one arm the named `destination` sits in, and takes the destination's
//! position there. On every traversal that leaves through a different edge
//! the member no longer executes at all.
//!
//! Skipping the member on the other paths is sound only when nothing there
//! can observe its absence. The member must be pure register and
//! condition-state work — no barrier kind, no call-roster entry, no memory
//! roster rows, and no memory-capable kind the roster leaves unaccounted —
//! because an access that happened on every traversal before the move would
//! happen only on the landing path after it. And every register and
//! condition-state unit the member writes must be dead along every path
//! leaving the branch's other edges: the forward dead-path audit walks the
//! successor region from each non-landing edge, reading each crossed edge's
//! register, structural-binding, and case-payload transports as boundary
//! positions, and refuses the moment a still-live member definition meets a
//! reader. A write retires the stale definition it replaces, so a location
//! that is redefined before any reader is legitimately dead even where the
//! member's own block loops back — the member's slot is simply absent from
//! the re-entered stream.
//!
//! The converging side needs no region structure at all: the landing arm is
//! any source block whose every predecessor edge leaves the member's own
//! terminator, so each traversal reaching the arm executes the member
//! exactly once — the same count it had inside the branch block. A second
//! predecessor into the arm would hand the arm's stream a member it never
//! ran before; a non-semantic landing edge or one carrying case custody,
//! fuel, or structural transfer would be a boundary effect the move skips
//! over, and both refuse.
//!
//! Between the member's old position and its new one lie only the crossed
//! positions the window audits: the member's own block tail behind it, the
//! branch terminator instruction, the landing edge's register transports,
//! and the arm's body instructions before the landing index. Positions
//! before the member's index, at or after the landing index, and in every
//! other block keep the member on the side they always had, so they are
//! never crossed. The non-landing edges are not crossed positions — the
//! member never traverses them — so their transports join only the
//! dead-path audit, where an argument reading a still-live member
//! definition refuses and a parameter writing one retires it.
//!
//! The hazard audit is the cross-edge family's applied across the fork: a
//! register or condition-state unit the member writes and a crossed
//! instruction reads or writes, or the member reads and a crossed
//! instruction writes, refuses in either direction. Calls, hosted effects,
//! terminator kinds, and call-roster entries are barriers anywhere in the
//! window — the branch terminator is the crossed edge's position, not a
//! window member — and a boundary settlement refuses where the member
//! changes sides with its block's executed prefix: past the member's index
//! in its own block, or past the landing index in the arm.
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

pub use rewrite::relocate_selected_instruction_into_arm;
pub use validation::validate_fork_relocation;

#[cfg(test)]
mod tests;

/// An accepted fork relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedForkRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ForkRelocationReceipt,
}

impl ValidatedForkRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &ForkRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl ForkRelocationReceipt {
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
pub enum ForkRelocationError {
    SourceMismatch,
    /// The named member can never move conditionally: a barrier kind, a
    /// call-roster entry, a roster-carrying or unaccounted memory-capable
    /// kind — a memory access that ran on every traversal would run only on
    /// the landing path — or a barrier kind, call, or unaccounted
    /// memory-capable instruction at a position the move would cross.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible fork window: the
    /// member's block lacks a two-successor conditional terminator, the
    /// destination names no position in a block the branch reaches, the
    /// landing arm is the member's own block, the entry block, a
    /// non-source implementation block, or reached by a predecessor other
    /// than the branch, a landing edge is not a plain semantic successor
    /// or carries case, fuel, or structural transfers, a register or
    /// condition-state hazard couples the member with a crossed position
    /// or the landing edge's transports, a boundary settlement observes a
    /// changed executed prefix, or the dead-path audit finds a register or
    /// unit the member writes still live at a reader on a path the member
    /// no longer executes on.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ForkRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid fork relocation: {self:?}")
    }
}

impl std::error::Error for ForkRelocationError {}
