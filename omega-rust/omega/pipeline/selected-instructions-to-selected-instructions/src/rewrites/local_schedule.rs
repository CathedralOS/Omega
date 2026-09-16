//! Local scheduling on the selected CFG.
//!
//! Scheduling here means changing the order instructions execute inside one
//! block without changing what any of them observes. The primitive this
//! family proves is the adjacent interchange: two neighboring body
//! instructions at positions `i` and `i+1` exchange places, the second
//! scheduled ahead of the first. Every other instruction, register, roster
//! row, call, settlement, and edge is retained bit-identical, and replayed
//! restore-by-content validation confirms the proposal is exactly that
//! interchange and nothing else.
//!
//! An interchange is admitted only when the pair is independent, which the
//! admission checks in both directions. A data hazard is any register or
//! condition-state unit one member writes and the other reads or also
//! writes: read/write sets come from the operand `access` marks plus the
//! implicit-use, implicit-def, and clobber lists, so a flag-defining compare
//! never crosses the boolean materialization that reads it, a flag reader
//! never crosses a later flag writer, and two writers of one location never
//! exchange. Third parties are covered by the same conditions: for any
//! location at most one member writes, so the last writer before every
//! later observer — including the block's terminator and edge transports —
//! is the same instruction it was before the swap, and positions before the
//! pair are untouched.
//!
//! Positional observers bound the window the same way store motion's walk
//! does. A boundary settlement at the interior index sits between the pair
//! and would see the later instruction run first, so it refuses; settlements
//! at or outside the pair's span observe the same executed set either way.
//! Calls, hosted effects, and terminator kinds are barriers and never
//! interchange, and an instruction named by the call roster refuses for the
//! same reason.
//!
//! Memory ordering follows the validated `memory_accesses` roster: a kind
//! that can reach semantic or place-backed storage must carry the rows
//! accounting for that reach. A pair is admitted when at most one member
//! carries rows — the row-less member then cannot observe memory at all
//! under the roster's completeness, so the relative order of every recorded
//! access in the program is preserved. Two row-carrying accesses would need
//! a place-alias decision this step does not take, and a memory-capable
//! kind without rows is an unaccounted access; both refuse.
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
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::schedule_selected_pair;
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
    /// The named instructions are not an admissible adjacent pair: not
    /// neighbors in that order, coupled by a register or condition-state
    /// hazard, both carrying memory-access rows, or separated by a boundary
    /// settlement at the interior position.
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
