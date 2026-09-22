//! Local run relocation on the selected CFG.
//!
//! Run relocation generalizes the single-member move to a multi-member
//! schedule: the named `first` and `last` members bound one contiguous run
//! of at least two body instructions in their block, the named
//! `destination` instruction sits outside the run, and the run takes the
//! destination's position as one body. The window the move crosses rotates
//! as a unit: toward an earlier destination the run lands on the window's
//! leading edge with the crossed instructions shifting one run-width later
//! in their original order, and toward a later destination the run lands
//! on the trailing edge with the crossed run shifting one run-width
//! earlier. A run of exactly one member is the single-member relocation
//! this family builds on and is refused here. Every instruction, register,
//! roster row, call, settlement, and edge outside the rotation is retained
//! bit-identical, and replayed restore-by-content validation confirms the
//! proposal is exactly that rotation and nothing else.
//!
//! The run moves as one body, so hazards inside it cannot change: two
//! members keep their relative order on either landing, and a producer
//! feeding its own consumer inside the run stays adjacent through a move
//! neither member could take alone — the member cannot cross the crossed
//! run while its partner waits behind. The hazard audit is therefore
//! one-sided against the crossed run: the members are the only
//! instructions whose position relative to a crossed position changes, so
//! a data hazard is any register or condition-state unit a member writes
//! and a crossed instruction reads or also writes, or a member reads and
//! a crossed instruction writes — the same access-mark plus implicit-use,
//! implicit-def, and clobber accounting the single-member relocation
//! proves, applied between every member and every crossed position.
//! Crossed instructions keep their relative order and members keep
//! theirs, so the last writer before every later observer — including the
//! block's terminator and edge transports — is the same instruction it
//! was before the move. Positions before the window are untouched.
//!
//! Positional observers bound the window the way the single-member move's
//! do. A boundary settlement at any position after the window's first
//! index through its last observes a different executed prefix once the
//! run lands — the run's members join the prefix from the trailing side or
//! leave it from the leading side — so a settlement inside the window's
//! span refuses; settlements at or outside the span observe the same
//! executed set either way. Calls, hosted effects, and terminator kinds
//! are barriers: they never relocate and never sit inside a crossed
//! window — as a run member or as a crossed position — and an instruction
//! named by the call roster refuses for the same reason.
//!
//! Memory ordering follows the validated `memory_accesses` roster with the
//! same completeness discipline as the single-member move: a kind that can
//! reach semantic or place-backed storage must carry the rows accounting
//! for that reach. The run trades order with every crossed position, so a
//! run carrying any roster rows may cross only row-less positions — a
//! second accounted actor anywhere in the window would need a place-alias
//! decision this step does not take. Two roster-carrying members inside
//! the run keep their own recorded order, so a multi-actor run is not a
//! second actor; it is one moving sequence whose accesses retain their
//! relative order. A row-less run cannot observe memory at all under the
//! roster's completeness, so it crosses any mix of accounted and interior
//! positions while every recorded access in the program keeps its
//! relative order. A memory-capable kind without rows is an unaccounted
//! access and refuses as a member or as a crossed instruction.
//!
//! No register roster row, constraint key, or instruction identity
//! changes: every member keeps its own id, kind, operands, provenance, and
//! implicit surface while only the run's position in the block vector
//! moves.
//!
//! Proposal and independent replay share only the admission predicates.
//! Validation consumes the proposed program, requires the touched block to
//! equal the independently computed rotation, and restores the complete
//! source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub use rewrite::relocate_selected_run;
pub use validation::validate_run_relocation;

#[cfg(test)]
mod tests;

/// An accepted run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRunRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: RunRelocationReceipt,
}

impl ValidatedRunRelocation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        Arc::clone(&self.transformed)
    }

    pub const fn receipt(&self) -> &RunRelocationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

impl RunRelocationReceipt {
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
pub enum RunRelocationError {
    SourceMismatch,
    UnsupportedInstruction,
    /// The named instructions do not bound an admissible in-block run and
    /// window: absent or coincident members, a last member that does not
    /// follow the first in the same block, a destination inside or outside
    /// the member's block or inside the run, a register or
    /// condition-state hazard between a member and a crossed instruction,
    /// a roster-carrying run sharing the window with a second
    /// memory-access actor, or a boundary settlement inside the window's
    /// span.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for RunRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid local run relocation: {self:?}")
    }
}

impl std::error::Error for RunRelocationError {}
