//! Instruction relocation out of a converging join, upstream through the
//! branch diamond that feeds it, on the selected CFG.
//!
//! The landed cross-boundary families all move a member downstream:
//! `rewrites/edge_relocation` crosses a `Jump` edge into the
//! sole-predecessor target, `rewrites/diamond_relocation` sinks a member
//! out of the branching head through both arms into the one join they
//! feed, and `rewrites/fork_relocation` sinks a member into the one arm
//! the named edge selects. This family is the first move upstream: the
//! named `member` leaves the converging join block's body, crosses every
//! arm and its `Jump`, crosses the branch's plain edges, and takes the
//! named `destination` position in the one fork head the arms descend
//! from.
//!
//! Rising upstream flips the soundness burden the fork move carries. A
//! sinking member stops executing on the paths its landing edge skips, so
//! every location it writes must be dead along them. A hoisting member
//! keeps executing exactly once per join traversal, so every path into
//! the join must supply it — total, not partial: every edge into the join
//! must leave an arm whose only predecessors are the fork head's own
//! edges, and every edge the head's conditional terminator names must
//! reach an arm. Then each traversal reaching the join traversed the
//! head, where the member now runs, and each traversal of the head
//! reaches the join, where it ran before — the execution count is one on
//! both sides and no fixpoint audit is needed. A second predecessor into
//! the join would hand the join's readers a member that never ran on that
//! path; a fork edge leaving the region would run the member on a
//! traversal the join never saw. Both refuse.
//!
//! The member's value obligations are the window audit's: its reads must
//! observe the same values at the head's position they observed in the
//! join, so no crossed position — head suffix, branch terminator, either
//! branch edge's transports, either arm's body, either arm `Jump`, either
//! join-ward edge's transports, or the join prefix behind the member —
//! may write a location it reads, and none may read or write a location
//! it writes. A transported register on any crossed edge binds the same
//! rule: a member defining the argument would hand the binding a new
//! value where the source bound the old, a member defining the parameter
//! would be overwritten before the join sees it, and a member reading the
//! parameter would observe the transported value only before the move.
//!
//! Calls, hosted effects, terminator kinds, and call-roster entries are
//! barriers anywhere in the window — the branch and arm terminators are
//! crossed positions, not window members — and a boundary settlement
//! refuses where the member changes sides with its block's executed
//! prefix: past the member's index in the join, or past the landing index
//! in the head. Arm blocks need no settlement audit: the member never
//! enters an arm's body, so no arm prefix ever contained or loses it.
//! Memory motion stays the shared accounting: a roster-carrying member
//! crosses only row-less positions, each terminator and each edge's own
//! access rows counting as accounted boundary positions.
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

pub use rewrite::relocate_selected_instruction_out_of_join;
pub use validation::validate_join_relocation;

#[cfg(test)]
mod tests;

/// An accepted join relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedJoinRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: JoinRelocationReceipt,
}

impl ValidatedJoinRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &JoinRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl JoinRelocationReceipt {
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
pub enum JoinRelocationError {
    SourceMismatch,
    /// The named member can never leave the converging side: a barrier
    /// kind, a call-roster entry, or an unaccounted memory-capable kind —
    /// or a barrier kind, call, or unaccounted memory-capable instruction
    /// at a position the move would cross.
    UnsupportedInstruction,
    /// The named pair does not bound an admissible join window: the
    /// member's block is not a source-origin join reached only by arms
    /// that end in a plain `Jump` to it, an arm is the join itself, the
    /// entry block, or a non-source block, an arm is reached by an edge
    /// leaving a block other than the one fork head, the head lacks a
    /// two-successor conditional terminator or names an edge that misses
    /// every arm, a crossed edge is not a plain semantic successor or
    /// carries case, fuel, or structural transfers, a register or
    /// condition-state hazard couples the member with a crossed position
    /// or a crossed edge's transports, a roster-carrying member shares the
    /// window with a second memory actor, or a boundary settlement
    /// observes a changed executed prefix.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for JoinRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid join relocation: {self:?}")
    }
}

impl std::error::Error for JoinRelocationError {}
