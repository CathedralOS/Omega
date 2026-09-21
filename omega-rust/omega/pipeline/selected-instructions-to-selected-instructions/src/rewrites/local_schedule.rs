//! Local scheduling on the selected CFG.
//!
//! Scheduling here means changing the order instructions execute inside one
//! block without changing what any of them observes. The primitive this
//! family proves is the bounded interchange: two body instructions at
//! positions `i` and `j` exchange places — the later scheduled ahead of the
//! earlier — while every instruction between them keeps its position. An
//! adjacent pair is the distance-one case. Every other instruction,
//! register, roster row, call, settlement, and edge is retained
//! bit-identical, and replayed restore-by-content validation confirms the
//! proposal is exactly that interchange and nothing else.
//!
//! An interchange is admitted only when the window the pair bounds is
//! independent, which the admission checks in both directions. A data hazard
//! is any register or condition-state unit one instruction writes and a
//! crossed instruction reads or also writes: read/write sets come from the
//! operand `access` marks plus the implicit-use, implicit-def, and clobber
//! lists, so a flag-defining compare never crosses the boolean
//! materialization that reads it, a flag reader never crosses a later flag
//! writer, and two writers of one location never exchange. The check runs
//! between the two named members and between each member and every
//! instruction inside the window, since the pair trades order with all of
//! them. Third parties are covered by the same conditions: for any location
//! at most one window member writes, so the last writer before every later
//! observer — including the block's terminator and edge transports — is the
//! same instruction it was before the swap, and positions before the
//! pair are untouched.
//!
//! Positional observers bound the window the same way store motion's walk
//! does. A boundary settlement inside the window's span would see the later
//! instruction run before the earlier one and every crossed position in a
//! different executed set, so it refuses; settlements at or outside the
//! span observe the same executed set either way. Calls, hosted effects,
//! and terminator kinds are barriers: they never interchange and never sit
//! inside a crossed window, and an instruction named by the call roster
//! refuses for the same reason.
//!
//! Memory ordering follows the validated `memory_accesses` roster: a kind
//! that can reach semantic or place-backed storage must carry the rows
//! accounting for that reach. A window is admitted when at most one of its
//! members carries rows and, when one does, no instruction inside the
//! window carries any — the row-less members then cannot observe memory at
//! all under the roster's completeness, so the relative order of every
//! recorded access in the program is preserved. An interior access keeps
//! its position and may stay accounted while two memory-inert members
//! trade places around it. Two row-carrying accesses exchanging order
//! would need a place-alias decision this step does not take, and a
//! memory-capable kind without rows is an unaccounted access; both
//! refuse.
//!
//! No register roster row, constraint key, or instruction identity changes:
//! each instruction keeps its own id, kind, operands, provenance, and
//! implicit surface while only its position in the block vector moves.
//!
//! Proposal and independent replay share only the admission predicates.
//! Validation consumes the proposed program, requires the touched block to
//! equal the independently computed interchange, and restores the complete
//! source by content.

mod admission;
mod rewrite;
mod scheduled_relocation;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::schedule_selected_pair;
pub use scheduled_relocation::{
    ScheduledRelocationError, ScheduledRelocationReceipt, ValidatedScheduledRelocation,
    relocate_scheduled_run, validate_scheduled_relocation,
};
pub use validation::validate_local_schedule;

#[cfg(test)]
mod tests;

/// An accepted adjacent interchange with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLocalSchedule {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: LocalScheduleReceipt,
}

impl ValidatedLocalSchedule {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &LocalScheduleReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalScheduleReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl LocalScheduleReceipt {
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
pub enum LocalScheduleError {
    SourceMismatch,
    UnsupportedInstruction,
    /// The named instructions are not an admissible in-block pair: absent
    /// or misordered in the earlier member's block, coupled by a register
    /// or condition-state hazard with each other or a crossed interior
    /// instruction, sharing the window with a second memory-access actor,
    /// or separated by a boundary settlement inside the window's span.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for LocalScheduleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid local schedule interchange: {self:?}")
    }
}

impl std::error::Error for LocalScheduleError {}
