//! Scheduling relocation admission: one mechanism over a member run and a
//! destination point on the selected CFG.
//!
//! The cross-boundary families — `rewrites/edge_relocation`,
//! `rewrites/fork_relocation`, `rewrites/diamond_relocation`, and the
//! converging and bypassed shapes beside them — each locate their own
//! window: one shape per family, each repeated for a contiguous run. This
//! admission replaces the per-shape location logic with the path-derived
//! window the families share: given a contiguous run of members in one
//! block's body and a destination position in a different block, it derives
//! every simple control-flow path between them, and from those paths the
//! crossed positions (the tail or head of the run's own block, each
//! traversed block's whole stream, and the destination block's share on
//! the far side of the landing index), the crossed edges, the skipped
//! edges that begin the traversals a sink no longer executes on, and the
//! traversals that would gain or lose the run — refused outright by the
//! once-per-traversal bound in either direction: sinking requires the
//! member's block to dominate the destination's, hoisting mirrors it —
//! the destination's block must dominate the member's, the member's
//! block must not be re-enterable without crossing the destination
//! again, and every continuation off the destination must reach the
//! member's block before it can exit or cycle. The hazard, dead-path,
//! and settlement audits then apply once to that derived window,
//! independent of which shape realized it.
//!
//! A window any per-shape family locates — the single `Jump` edge, the
//! fork's landing arm, the diamond's join — is a special case of the
//! derived region; windows the families cannot name, such as a run sinking
//! through a chain of sole-successor blocks or through one arm of a fork
//! into a deeper dominated block, or a run hoisting out of a join back
//! through its predecessors, are the shapes this admission exists to
//! cover. Moves where neither block dominates the other — across an
//! inflow or a branch the run would gain or lose on some traversal — and
//! in-block interchanges remain with the existing families and
//! `super::admission`.
//!
//! The member run must be pure register and condition-state work whose
//! written locations die on every path a sink abandons, the window's
//! crossed positions must be schedulable and uncoupled from every member,
//! every crossed edge must be a plain semantic successor the run does not
//! interfere with, no boundary settlement may observe a changed executed
//! prefix, and every block on a crossing path must be a plain `Source`
//! block. Validation consumes the proposed program, requires the run to
//! sit contiguous at the landing index in the destination block, and
//! restores the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_scheduled_run;
pub use validation::validate_scheduled_relocation;

#[cfg(test)]
mod tests;

/// An accepted scheduled relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedScheduledRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ScheduledRelocationReceipt,
}

impl ValidatedScheduledRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &ScheduledRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl ScheduledRelocationReceipt {
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
pub enum ScheduledRelocationError {
    /// The named member run or destination does not resolve inside the
    /// named function, or the plan's target does not match the register
    /// environment's.
    SourceMismatch,
    /// The named run can never move past a block boundary: a barrier kind,
    /// a call-roster entry, a roster-carrying or unaccounted memory-capable
    /// kind — an access or fault that ran on every traversal would run only
    /// on the landing path — or a barrier kind, call, or unaccounted
    /// memory-capable instruction at a position the move would cross.
    UnsupportedInstruction,
    /// The named run and destination do not bound an admissible window:
    /// the run is empty, non-contiguous, or shares the destination's
    /// block; the destination names no position, or neither block
    /// dominates the other; the destination (sinking) is reachable from
    /// its own successors, or the run's block (hoisting) is re-enterable
    /// without crossing the destination again, or a continuation off the
    /// destination can exit or cycle before reaching the run's block; a
    /// block on a crossing path is not a plain `Source` block; a crossed
    /// edge is not a plain semantic successor or carries case, fuel, or
    /// structural transfers; a register or condition-state hazard couples
    /// a member with a crossed position or a crossed edge's transports; a
    /// boundary settlement observes a changed executed prefix; or the
    /// dead-path audit finds a location a member writes still live at a
    /// reader on a path the run no longer executes on.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ScheduledRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid scheduled relocation: {self:?}")
    }
}

impl std::error::Error for ScheduledRelocationError {}
