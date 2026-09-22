//! Local scheduling of whole runs on the selected CFG.
//!
//! Scheduling here means changing the order instructions execute inside one
//! block without changing what any of them observes. The primitive this
//! family proves is the bounded run interchange: two disjoint runs of body
//! instructions — each the contiguous span its named `earlier` or `later`
//! first and last members bound, of at least two members — exchange places
//! while every instruction between them keeps its relative order, shifting
//! by the difference of the runs' lengths. The interchange completes the
//! scheduling family's two-by-two of bounded moves:
//! `rewrites/local_schedule` interchanges two members,
//! `rewrites/local_relocation` moves one member across the window it
//! spans, `rewrites/run_relocation` moves one run, and this family trades
//! two runs. An adjacent pair of runs is the empty-interior case and
//! coincides with the rotation `run_relocation` proves; a one-member run
//! is the pair interchange's or the member relocation's granularity and
//! refuses, as does a singleton pair, which is `local_schedule`'s own
//! shape. Every other instruction, register, roster row, call, settlement,
//! and edge is retained bit-identical, and replayed restore-by-content
//! validation confirms the proposal is exactly that interchange and
//! nothing else.
//!
//! An interchange is admitted only when the window the runs bound is
//! independent. The members of a run keep their relative order — the run
//! moves as one body — so a data hazard is any register or
//! condition-state unit a member writes and an instruction outside its own
//! run inside the window reads or also writes, or the member reads and the
//! outside position writes: the same access-mark plus implicit-use,
//! implicit-def, and clobber accounting the pair interchange proves,
//! applied between every member and every crossed position — the other
//! run's members and the whole interior between the runs. Interior
//! positions keep their relative order with each other, positions outside
//! the window keep theirs with every member, and since no location is
//! written by both a member and a crossed position the last writer before
//! every later observer — including the block's terminator and edge
//! transports — is the same instruction it was before the interchange.
//! Positions before the window are untouched.
//!
//! Positional observers bound the window the same way the pair
//! interchange's does. A boundary settlement at any position after the
//! earlier run's first index through the later run's last observes a
//! different executed prefix once the runs trade places, so it refuses;
//! settlements at or outside the span observe the same executed set either
//! way. Calls, hosted effects, and terminator kinds are barriers: they
//! never interchange and never sit inside a crossed window, and an
//! instruction named by the call roster refuses for the same reason.
//!
//! Memory ordering follows the validated `memory_accesses` roster with the
//! same completeness discipline as the run relocation's: a kind that can
//! reach semantic or place-backed storage must carry the rows accounting
//! for that reach. Roster-carrying members of one run keep their relative
//! order, so a run may carry several rows itself, but every position it
//! trades order with — the other run's members and the whole interior —
//! must then be row-less: a second accounted actor the run crosses would
//! reorder recorded accesses. When neither run carries rows the interior's
//! own accounted positions keep their relative order and admit. A
//! memory-capable kind without rows is an unaccounted access and refuses.
//!
//! No register roster row, constraint key, or instruction identity
//! changes: each instruction keeps its own id, kind, operands, provenance,
//! and implicit surface while only its position in the block vector moves.
//!
//! Proposal and independent replay share only the admission predicates.
//! Validation consumes the proposed program, requires the touched block's
//! window to equal the independently computed interchange, and restores
//! the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::interchange_selected_runs;
pub use validation::validate_run_interchange;

#[cfg(test)]
mod tests;

/// An accepted run interchange with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRunInterchange {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: RunInterchangeReceipt,
}

impl ValidatedRunInterchange {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &RunInterchangeReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunInterchangeReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl RunInterchangeReceipt {
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
pub enum RunInterchangeError {
    SourceMismatch,
    UnsupportedInstruction,
    /// The named instructions do not bound two admissible runs in one
    /// block: absent, a one-member run, misordered, or overlapping; a
    /// member coupled by a register or condition-state hazard with a
    /// crossed position; a roster-carrying run sharing the window with a
    /// second memory-access actor; or a boundary settlement inside the
    /// window's span.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for RunInterchangeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid local run interchange: {self:?}")
    }
}

impl std::error::Error for RunInterchangeError {}
