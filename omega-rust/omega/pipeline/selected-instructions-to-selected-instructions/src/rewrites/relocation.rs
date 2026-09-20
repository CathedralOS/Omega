//! Member-run relocation through one shared admission on the selected CFG.
//!
//! The per-shape scheduling families — `edge`, `predecessor`, `diamond`,
//! `join`, `fork`, `arm`, `bypass`, `triangle`, `confluence`, `inflow` and
//! their run variants — each enumerate the window their own shape crosses
//! and spell the independence audit by hand. This module is the general
//! mechanism the execution board's scheduling flag names: one admission
//! over a contiguous member run and a destination point. Given
//! `first_member`..`last_member` in one block's body and a `destination`
//! instruction anywhere in the function, admission derives the window the
//! move crosses itself — every acyclic edge path between the run's block
//! and the destination block contributes its terminator-carried
//! instruction and successor row, every intermediate block contributes
//! its whole body, the run's own block contributes the tail behind the
//! run, and the destination block contributes the positions before the
//! landing index — then proves the move changes no traversal: every
//! predecessor edge into the destination lies on a crossed path (no
//! traversal gains the run) and every exit of every block any path
//! leaves reaches the destination (no traversal loses the run). The
//! shared audit then runs once: every member schedulable, no member
//! coupled with a crossed position or a crossed edge's terminator
//! instruction, every crossed edge a plain semantic successor free of
//! register-transport interference, roster-carrying runs crossing only
//! unaccounted positions, and no boundary settlement whose observed
//! executed prefix changes — including settlements inside fully crossed
//! intermediate bodies.
//!
//! A run named inside the same block it lands in is the in-block move the
//! local families own: the window is the span between the run and the
//! landing index and the traversal proof is vacuous. Moves whose gained
//! or lost traversal set is nonempty but still sound — the fork-head
//! hoists and join sinks — are the `dead_path` audit's families and stay
//! with them; this admission proves the preservation case.
//!
//! No instruction, register, roster row, call, settlement, successor, or
//! terminator record changes: every member keeps its own id, kind,
//! operands, provenance, and implicit surface while only the run's
//! position in the program moves. Proposal and independent replay share
//! only the admission. Validation consumes the proposed program,
//! requires the run to sit at the landing index in its original order,
//! and restores the complete source by content — every other block,
//! instruction, register, roster row, call, and settlement is retained
//! bit-identical.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_member_run;
pub use validation::validate_member_run_relocation;

#[cfg(test)]
mod tests;

/// An accepted member-run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMemberRunRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: MemberRunRelocationReceipt,
}

impl ValidatedMemberRunRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &MemberRunRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRunRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl MemberRunRelocationReceipt {
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
pub enum MemberRunRelocationError {
    SourceMismatch,
    /// A member or a crossed position can never trade order: a barrier
    /// kind, a call-roster entry, or an unaccounted memory reach.
    UnsupportedInstruction,
    /// The named span or destination does not bound an admissible
    /// relocation: the last member does not close a contiguous run after
    /// the first in one block, the destination names no position, the
    /// move crosses no acyclic path to the destination, the destination
    /// is the entry block or not a plain source block, a traversal would
    /// gain or lose the run (a destination predecessor outside the
    /// crossed paths, or an exit of a crossed block that never reaches
    /// the destination), the run would land inside its own span, a
    /// crossed edge carries boundary effects, a register or
    /// condition-state hazard couples a member with a crossed position,
    /// a member interferes with a crossed edge's register transports, a
    /// second memory-access actor shares the window, or a boundary
    /// settlement observes a changed executed prefix.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for MemberRunRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid member-run relocation: {self:?}")
    }
}

impl std::error::Error for MemberRunRelocationError {}
